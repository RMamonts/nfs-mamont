use nfs_mamont::vfs::{self, read_dir};

use super::MirrorFS;

impl read_dir::ReadDir for MirrorFS {
    async fn read_dir(&self, args: read_dir::Args) -> Result<read_dir::Success, read_dir::Fail> {
        let dir_path = match self.path_for_handle(&args.dir).await {
            Ok(path) => path,
            Err(error) => return Err(read_dir::Fail { error, dir_attr: None }),
        };
        let dir_attr = match self.attr_for_path(&dir_path).await {
            Ok(attr) => attr,
            Err(error) => return Err(read_dir::Fail { error, dir_attr: None }),
        };
        if let Err(error) = Self::validate_directory(&dir_attr) {
            return Err(read_dir::Fail { error, dir_attr: Some(dir_attr) });
        }

        let verifier = Self::cookie_verifier_for_attr(&dir_attr);
        if !args.cookie.is_zero() && args.cookie_verifier != verifier {
            return Err(read_dir::Fail { error: vfs::Error::BadCookie, dir_attr: Some(dir_attr) });
        }

        let entries = match self.directory_entries_for(&dir_path, verifier).await {
            Ok(entries) => entries,
            Err(error) => return Err(read_dir::Fail { error, dir_attr: Some(dir_attr) }),
        };

        let total_entries = entries.len();
        let start = args.cookie.raw() as usize;
        let mut used = 0u32;
        let mut result = Vec::new();
        let mut selected_paths = Vec::new();
        for (index, entry) in entries.iter().enumerate().skip(start) {
            let name = entry.name.clone();
            let estimated = (24 + name.as_str().len()) as u32;
            if !result.is_empty() && used.saturating_add(estimated) > args.count {
                break;
            }
            selected_paths.push(entry.path.clone());
            result.push(read_dir::Entry {
                file_id: entry.file_id,
                file_name: name,
                cookie: read_dir::Cookie::new((index + 1) as u64),
            });
            used = used.saturating_add(estimated);
        }

        self.cache_handles_for_paths(&selected_paths).await;

        let eof = start >= total_entries || start.saturating_add(result.len()) >= total_entries;

        Ok(read_dir::Success {
            dir_attr: Some(dir_attr),
            cookie_verifier: verifier,
            entries: result,
            eof,
        })
    }
}
