use super::MirrorFS;
use crate::cache::readdir::{DirectoryEntrySnapshot, DirectoryListingSnapshot};
use nfs_mamont::vfs::read_dir::Entry;
use nfs_mamont::vfs::{self, read_dir};
use std::sync::Arc;

impl read_dir::ReadDir for MirrorFS {
    async fn read_dir(&self, args: read_dir::Args) -> Result<read_dir::Success, read_dir::Fail> {
        let dir_path = match self.path_for_handle(&args.dir).await {
            Ok(path) => path,
            Err(error) => return Err(read_dir::Fail { error, dir_attr: None }),
        };
        let dir_meta = match Self::metadata(&dir_path) {
            Ok(meta) => meta,
            Err(error) => return Err(read_dir::Fail { error, dir_attr: None }),
        };
        let dir_attr = Self::attr_from_metadata(&dir_meta);
        if let Err(error) = Self::validate_directory(&dir_attr) {
            return Err(read_dir::Fail { error, dir_attr: Some(dir_attr) });
        }

        let verifier = Self::cookie_verifier_for_attr(&dir_attr);
        if !args.cookie.is_zero() && args.cookie_verifier != verifier {
            return Err(read_dir::Fail { error: vfs::Error::BadCookie, dir_attr: Some(dir_attr) });
        }

        if let Some(cached) = self.cache.read_dir_cache.look_for_cache(&args.dir).await {
            let entries = cached
                .entries
                .iter()
                .enumerate()
                .map(|(index, entry)| read_dir::Entry {
                    file_id: entry.file_id,
                    file_name: entry.name.clone(),
                    cookie: read_dir::Cookie::new((index + 1) as u64),
                })
                .collect::<Vec<Entry>>();
            let start = args.cookie.raw() as usize;
            let eof =
                start >= entries.len() || start.saturating_add(entries.len()) >= entries.len();
            return Ok(read_dir::Success {
                dir_attr: Some(dir_attr),
                cookie_verifier: verifier,
                entries,
                eof,
            });
        }

        let entries = match self.list_directory_entries(&dir_path) {
            Ok(entries) => entries,
            Err(error) => return Err(read_dir::Fail { error, dir_attr: Some(dir_attr) }),
        };

        let total_entries = entries.len();
        let start = args.cookie.raw() as usize;
        let mut used = 0u32;
        let mut result = Vec::new();
        for (index, (name, meta)) in entries.into_iter().enumerate().skip(start) {
            let estimated = (24 + name.as_str().len()) as u32;
            if !result.is_empty() && used.saturating_add(estimated) > args.count {
                break;
            }
            let attr = Self::attr_from_metadata(&meta);
            let path = dir_path.join(name.as_str());
            let _ = self.handle_for_path(&path).await;
            result.push(read_dir::Entry {
                file_id: attr.file_id,
                file_name: name,
                cookie: read_dir::Cookie::new((index + 1) as u64),
            });
            used = used.saturating_add(estimated);
        }
        let eof = start >= total_entries || start.saturating_add(result.len()) >= total_entries;

        let snapshot = Arc::new(DirectoryListingSnapshot {
            verifier,
            entries: Arc::new(
                result
                    .iter()
                    .map(|entry| DirectoryEntrySnapshot {
                        name: entry.file_name.clone(),
                        file_id: entry.file_id,
                    })
                    .collect(),
            ),
        });

        // set cache for future use
        self.cache.read_dir_cache.add_entry(&args.dir, snapshot.clone()).await;

        Ok(read_dir::Success {
            dir_attr: Some(dir_attr),
            cookie_verifier: verifier,
            entries: result,
            eof,
        })
    }
}
