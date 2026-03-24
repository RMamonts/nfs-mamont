use nfs_mamont::consts::nfsv3::NFS3_WRITEVERFSIZE;
use nfs_mamont::vfs::{self, read_dir, read_dir_plus};

use super::MirrorFS;

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

        let entries = match self.list_directory_entries(&dir_path) {
            Ok(entries) => entries,
            Err(error) => return Err(read_dir_plus::Fail { error, dir_attr: Some(dir_attr) }),
        };

        let total_entries = entries.len();
        let start = args.cookie.raw() as usize;
        let mut used = 0u32;
        let mut selected_paths = Vec::new();
        let mut selected_entries = Vec::new();
        for (index, (name, path, meta)) in entries.into_iter().enumerate().skip(start) {
            let estimated = (48 + name.as_str().len() + NFS3_WRITEVERFSIZE) as u32;
            if !selected_entries.is_empty() && used.saturating_add(estimated) > args.max_count {
                break;
            }
            let attr = Self::attr_from_metadata(&meta);
            selected_entries.push((
                attr.file_id,
                name,
                read_dir::Cookie::new((index + 1) as u64),
                attr,
            ));
            selected_paths.push(path);
            used = used.saturating_add(estimated);
        }

        let handles = match self.ensure_handles_for_paths(&selected_paths).await {
            Ok(handles) => handles,
            Err(error) => return Err(read_dir_plus::Fail { error, dir_attr: Some(dir_attr) }),
        };

        let mut result = Vec::with_capacity(selected_entries.len());
        for ((file_id, file_name, cookie, file_attr), file_handle) in
            selected_entries.into_iter().zip(handles)
        {
            result.push(read_dir_plus::Entry {
                file_id,
                file_name,
                cookie,
                file_attr: Some(file_attr),
                file_handle: Some(file_handle),
            });
        }

        Ok(read_dir_plus::Success {
            dir_attr: Some(dir_attr),
            cookie_verifier: verifier,
            eof: start >= total_entries || start.saturating_add(result.len()) >= total_entries,
            entries: result,
        })
    }
}
