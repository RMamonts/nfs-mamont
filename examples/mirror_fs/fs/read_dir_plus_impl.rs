use async_trait::async_trait;
use std::path::Path;

use nfs_mamont::consts::nfsv3::NFS3_WRITEVERFSIZE;
use nfs_mamont::vfs::{self, read_dir, read_dir_plus};

use super::MirrorFS;

#[async_trait]
impl read_dir_plus::ReadDirPlus for MirrorFS {
    async fn read_dir_plus(
        &self,
        args: read_dir_plus::Args,
        path: &Path,
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

        let entries = match self.list_directory_entries(&dir_path) {
            Ok(entries) => entries,
            Err(error) => return Err(read_dir_plus::Fail { error, dir_attr: Some(dir_attr) }),
        };

        let start = args.cookie.raw() as usize;
        let mut used = 0u32;
        let mut result = Vec::new();
        for (index, (name, path, meta)) in entries.iter().cloned().enumerate().skip(start) {
            let estimated = (48 + name.as_str().len() + NFS3_WRITEVERFSIZE) as u32;
            if !result.is_empty() && used.saturating_add(estimated) > args.max_count {
                break;
            }
            let attr = Self::attr_from_metadata(&meta);
            let handle = match self.ensure_handle_for_path(&path).await {
                Ok(handle) => handle,
                Err(error) => return Err(read_dir_plus::Fail { error, dir_attr: Some(dir_attr) }),
            };
            result.push(read_dir_plus::Entry {
                file_id: attr.file_id,
                file_name: name,
                cookie: read_dir::Cookie::new((index + 1) as u64),
                file_attr: Some(attr),
                file_handle: Some(handle),
            });
            used = used.saturating_add(estimated);
        }

        Ok(read_dir_plus::Success {
            dir_attr: Some(dir_attr),
            cookie_verifier: verifier,
            eof: start >= entries.len() || start.saturating_add(result.len()) >= entries.len(),
            entries: result,
        })
    }
}
