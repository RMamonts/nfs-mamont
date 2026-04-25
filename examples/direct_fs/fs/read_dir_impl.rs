use async_trait::async_trait;

use nfs_mamont::vfs::{self, read_dir};

use super::MirrorFS;

#[async_trait]
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

        let entries = match self.list_directory_entries(&dir_path) {
            Ok(entries) => entries,
            Err(error) => return Err(read_dir::Fail { error, dir_attr: Some(dir_attr) }),
        };

        let total_entries = entries.len();
        let start = args.cookie.raw() as usize;
        let mut used = 0u32;
        let mut result = Vec::new();
        for (index, (name, path, meta)) in entries.into_iter().enumerate().skip(start) {
            let estimated = (24 + name.as_str().len()) as u32;
            if !result.is_empty() && used.saturating_add(estimated) > args.count {
                break;
            }
            let attr = Self::attr_from_metadata(&meta);
            let _ = self.ensure_handle_for_path(&path).await;
            result.push(read_dir::Entry {
                file_id: attr.file_id,
                file_name: name,
                cookie: read_dir::Cookie::new((index + 1) as u64),
            });
            used = used.saturating_add(estimated);
        }
        let eof = start >= total_entries || start.saturating_add(result.len()) >= total_entries;

        Ok(read_dir::Success {
            dir_attr: Some(dir_attr),
            cookie_verifier: verifier,
            entries: result,
            eof,
        })
    }
}
