use tokio::fs;

use nfs_mamont::vfs::{self, rm_dir};

use super::MirrorFS;

impl rm_dir::RmDir for MirrorFS {
    async fn rm_dir(&self, args: rm_dir::Args) -> Result<rm_dir::Success, rm_dir::Fail> {
        if args.object.name.as_str() == "." {
            return Err(rm_dir::Fail {
                error: vfs::Error::InvalidArgument,
                dir_wcc: vfs::WccData { before: None, after: None },
            });
        }
        if args.object.name.as_str() == ".." {
            return Err(rm_dir::Fail {
                error: vfs::Error::InvalidArgument,
                dir_wcc: vfs::WccData { before: None, after: None },
            });
        }

        let dir_path = match self.path_for_handle(&args.object.dir).await {
            Ok(path) => path,
            Err(error) => {
                return Err(rm_dir::Fail {
                    error,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = std::fs::symlink_metadata(&dir_path)
            .ok()
            .map(|meta| Self::wcc_attr_from_metadata(&meta));
        let child_path = match self.child_path(&args.object.dir, &args.object.name).await {
            Ok(path) => path,
            Err(error) => {
                return Err(rm_dir::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) })
            }
        };
        let child_meta = match Self::metadata(&child_path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(rm_dir::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) })
            }
        };
        if !child_meta.is_dir() {
            return Err(rm_dir::Fail {
                error: vfs::Error::NotDir,
                dir_wcc: Self::wcc_data(&dir_path, before),
            });
        }

        match fs::remove_dir(&child_path).await {
            Ok(()) => {
                self.remove_cached_path(&child_path).await;
                self.invalidate_attr_cache_path(&dir_path).await;
                Ok(rm_dir::Success { wcc_data: Self::wcc_data(&dir_path, before) })
            }
            Err(error) => Err(rm_dir::Fail {
                error: Self::io_error_to_vfs(&error),
                dir_wcc: Self::wcc_data(&dir_path, before),
            }),
        }
    }
}
