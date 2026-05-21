use super::MirrorFS;
use crate::cache::readdir::DirectoryListingSnapshot;
use nfs_mamont::consts::nfsv3::NFS3_WRITEVERFSIZE;
use nfs_mamont::vfs::{self, file, read_dir, read_dir_plus};
use std::sync::Arc;

impl read_dir_plus::ReadDirPlus for MirrorFS {
    async fn read_dir_plus(
        &self,
        args: read_dir_plus::Args,
    ) -> Result<read_dir_plus::Success, read_dir_plus::Fail> {
        let dir_path = match self.path_for_handle(&args.dir).await {
            Ok(path) => path,
            Err(error) => return Err(read_dir_plus::Fail { error, dir_attr: None }),
        };
        let dir_meta = match Self::metadata(&dir_path) {
            Ok(meta) => meta,
            Err(error) => return Err(read_dir_plus::Fail { error, dir_attr: None }),
        };
        let dir_attr = Self::attr_from_metadata(&dir_meta);
        if let Err(error) = Self::validate_directory(&dir_attr) {
            return Err(read_dir_plus::Fail { error, dir_attr: Some(dir_attr) });
        }

        let verifier = Self::cookie_verifier_for_attr(&dir_attr);
        if !args.cookie.is_zero() && args.cookie_verifier != verifier {
            return Err(read_dir_plus::Fail {
                error: vfs::Error::BadCookie,
                dir_attr: Some(dir_attr),
            });
        }

        let entries = if let Some(cached) =
            self.cache.read_dir_cache.look_for_cache(&args.dir).await
        {
            cached.entries.iter().cloned().collect::<Vec<file::Name>>()
        } else {
            match self.list_directory_entries(&dir_path) {
                Ok(entries) => entries,
                Err(error) => return Err(read_dir_plus::Fail { error, dir_attr: Some(dir_attr) }),
            }
        };

        let start = args.cookie.raw() as usize;
        let mut used = 0u32;
        let mut result = Vec::new();
        for (index, name) in entries.iter().enumerate().skip(start) {
            let estimated = (48 + name.as_str().len() + NFS3_WRITEVERFSIZE) as u32;
            if !result.is_empty() && used.saturating_add(estimated) > args.max_count {
                break;
            }
            let path = dir_path.join(name.as_str());
            let attr = match Self::file_attr(&path) {
                Some(attr) => attr,
                None => {
                    return Err(read_dir_plus::Fail {
                        error: vfs::Error::ServerFault,
                        dir_attr: Some(dir_attr),
                    })
                }
            };
            let handle = match self.handle_for_path(&path).await {
                Ok(handle) => handle,
                Err(error) => return Err(read_dir_plus::Fail { error, dir_attr: Some(dir_attr) }),
            };
            result.push(read_dir_plus::Entry {
                file_id: attr.file_id,
                file_name: name.clone(),
                cookie: read_dir::Cookie::new((index + 1) as u64),
                file_attr: Some(attr),
                file_handle: Some(handle),
            });
            used = used.saturating_add(estimated);
        }

        let eof = start >= entries.len() || start.saturating_add(result.len()) >= entries.len();

        let snapshot = Arc::new(DirectoryListingSnapshot { verifier, entries: Arc::new(entries) });

        // set cache for future use
        self.cache.read_dir_cache.add_entry(&args.dir, snapshot.clone()).await;

        Ok(read_dir_plus::Success {
            dir_attr: Some(dir_attr),
            cookie_verifier: verifier,
            eof,
            entries: result,
        })
    }
}
