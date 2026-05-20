use tokio::fs;

use nfs_mamont::vfs::{self, file, remove};

use super::MirrorFS;

impl remove::Remove for MirrorFS {
    async fn remove(&self, args: remove::Args) -> Result<remove::Success, remove::Fail> {
        if let Err(error) = Self::ensure_name_allowed(&args.object.name) {
            return Err(remove::Fail {
                error,
                dir_wcc: vfs::WccData { before: None, after: None },
            });
        }

        let dir_path = match self.path_for_handle(&args.object.dir).await {
            Ok(path) => path,
            Err(error) => {
                return Err(remove::Fail {
                    error,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before =
            self.metadata(&dir_path).await.ok().map(|meta| Self::wcc_attr_from_statx(&meta));
        let child_path = match self.child_path(&args.object.dir, &args.object.name).await {
            Ok(path) => path,
            Err(error) => {
                return Err(remove::Fail {
                    error,
                    dir_wcc: self.wcc_data(&dir_path, before).await,
                });
            }
        };
        let child_meta = match self.metadata(&child_path).await {
            Ok(meta) => meta,
            Err(error) => {
                return Err(remove::Fail {
                    error,
                    dir_wcc: self.wcc_data(&dir_path, before).await,
                });
            }
        };
        if matches!(Self::attr_from_statx(&child_meta).file_type, file::Type::Directory) {
            return Err(remove::Fail {
                error: vfs::Error::IsDir,
                dir_wcc: self.wcc_data(&dir_path, before).await,
            });
        }

        if let Err(error) = fs::remove_file(&child_path).await {
            return Err(remove::Fail {
                error: Self::io_error_to_vfs(&error),
                dir_wcc: self.wcc_data(&dir_path, before).await,
            });
        }
        self.remove_cached_path(&child_path).await;

        Ok(remove::Success { wcc_data: self.wcc_data(&dir_path, before).await })
    }
}
