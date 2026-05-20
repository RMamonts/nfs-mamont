use nfs_mamont::vfs::{self, symlink};

use super::MirrorFS;

impl symlink::Symlink for MirrorFS {
    async fn symlink(&self, args: symlink::Args) -> Result<symlink::Success, symlink::Fail> {
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
        let before =
            self.metadata(&dir_path).await.ok().map(|meta| Self::wcc_attr_from_statx(&meta));
        let mut link_path = dir_path.clone();
        link_path.push(args.object.name.as_str());

        match std::os::unix::fs::symlink(args.path.as_path(), &link_path) {
            Ok(()) => {}
            Err(error) => {
                return Err(symlink::Fail {
                    error: Self::io_error_to_vfs(&error),
                    dir_wcc: self.wcc_data(&dir_path, before).await,
                });
            }
        }

        let attr = match self.metadata(&link_path).await {
            Ok(meta) => Self::attr_from_statx(&meta),
            Err(error) => {
                return Err(symlink::Fail {
                    error,
                    dir_wcc: self.wcc_data(&dir_path, before).await,
                })
            }
        };
        let handle = match self.handle_for_path(&link_path).await {
            Ok(handle) => handle,
            Err(error) => {
                return Err(symlink::Fail {
                    error,
                    dir_wcc: self.wcc_data(&dir_path, before).await,
                })
            }
        };

        Ok(symlink::Success {
            file: Some(handle),
            attr: Some(attr),
            wcc_data: self.wcc_data(&dir_path, before).await,
        })
    }
}
