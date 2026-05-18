use nfs_mamont::vfs::{self, remove};

use crate::async_fs;

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

        let before = async_fs::symlink_metadata(&dir_path).await
            .ok()
            .map(|meta| Self::wcc_attr_from_metadata(&meta));

        let child_path = match self.child_path(&args.object.dir, &args.object.name).await {
            Ok(path) => path,
            Err(error) => {
                return Err(remove::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) });
            }
        };

        let child_meta = match async_fs::symlink_metadata(&child_path).await {
            Ok(meta) => meta,
            Err(error) => {
                return Err(remove::Fail { error: Self::io_error_to_vfs(&error), dir_wcc: Self::wcc_data(&dir_path, before) });
            }
        };

        if child_meta.is_dir() {
            return Err(remove::Fail {
                error: vfs::Error::IsDir,
                dir_wcc: Self::wcc_data(&dir_path, before),
            });
        }

        if let Err(error) = async_fs::remove_file(&child_path).await {
            return Err(remove::Fail {
                error: Self::io_error_to_vfs(&error),
                dir_wcc: Self::wcc_data(&dir_path, before),
            });
        }

        self.remove_cached_path(&child_path).await;

        Ok(remove::Success { wcc_data: Self::wcc_data(&dir_path, before) })
    }
}