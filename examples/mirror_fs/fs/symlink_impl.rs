use async_trait::async_trait;

use nfs_mamont::vfs::symlink;

use super::*;

#[async_trait]
impl symlink::Symlink for MirrorFS {
    async fn symlink(&self, args: symlink::Args) -> symlink::Result {
        if let Err(error) = Self::ensure_name_allowed(&args.object.name) {
            return Err(symlink::Fail {
                error,
                dir_wcc: vfs::WccData { before: None, after: None },
            });
        }

        let dir_path = match self.path_for_handle(&args.object.dir).await {
            Ok(path) => path,
            Err(error) => {
                return Err(symlink::Fail {
                    error,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = std::fs::symlink_metadata(&dir_path)
            .ok()
            .map(|meta| Self::wcc_attr_from_metadata(&meta));
        let mut link_path = dir_path.clone();
        link_path.push(args.object.name.as_str());

        match std::os::unix::fs::symlink(args.path.as_path(), &link_path) {
            Ok(()) => {}
            Err(error) => {
                return Err(symlink::Fail {
                    error: Self::io_error_to_vfs(&error),
                    dir_wcc: Self::wcc_data(&dir_path, before),
                });
            }
        }

        let attr = match Self::metadata(&link_path) {
            Ok(meta) => Self::attr_from_metadata(&meta),
            Err(error) => {
                return Err(symlink::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) })
            }
        };
        let handle = match self.ensure_handle_for_path(&link_path).await {
            Ok(handle) => handle,
            Err(error) => {
                return Err(symlink::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) })
            }
        };

        Ok(symlink::Success {
            file: Some(handle),
            attr: Some(attr),
            wcc_data: Self::wcc_data(&dir_path, before),
        })
    }
}
