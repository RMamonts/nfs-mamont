use nfs_mamont::vfs::{self, file, rm_dir};

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
        let before =
            self.metadata(&dir_path).await.ok().map(|meta| Self::wcc_attr_from_statx(&meta));
        let child_path = match self.child_path(&args.object.dir, &args.object.name).await {
            Ok(path) => path,
            Err(error) => {
                return Err(rm_dir::Fail { error, dir_wcc: self.wcc_data(&dir_path, before).await })
            }
        };
        let child_meta = match self.metadata(&child_path).await {
            Ok(meta) => meta,
            Err(error) => {
                return Err(rm_dir::Fail { error, dir_wcc: self.wcc_data(&dir_path, before).await })
            }
        };
        if !matches!(Self::attr_from_statx(&child_meta).file_type, file::Type::Directory) {
            return Err(rm_dir::Fail {
                error: vfs::Error::NotDir,
                dir_wcc: self.wcc_data(&dir_path, before).await,
            });
        }

        match std::fs::remove_dir(&child_path) {
            Ok(()) => {
                self.remove_cached_path(&child_path).await;
                Ok(rm_dir::Success { wcc_data: self.wcc_data(&dir_path, before).await })
            }
            Err(error) => Err(rm_dir::Fail {
                error: Self::io_error_to_vfs(&error),
                dir_wcc: self.wcc_data(&dir_path, before).await,
            }),
        }
    }
}
