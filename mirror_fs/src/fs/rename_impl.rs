use nfs_mamont::vfs::{self, file, rename};

use super::MirrorFS;

impl rename::Rename for MirrorFS {
    async fn rename(&self, args: rename::Args) -> Result<rename::Success, rename::Fail> {
        if matches!(args.from.name.as_str(), "." | "..")
            || matches!(args.to.name.as_str(), "." | "..")
        {
            return Err(rename::Fail {
                error: vfs::Error::InvalidArgument,
                from_dir_wcc: vfs::WccData { before: None, after: None },
                to_dir_wcc: vfs::WccData { before: None, after: None },
            });
        }

        let from_dir_path = match self.path_for_handle(&args.from.dir).await {
            Ok(path) => path,
            Err(error) => {
                return Err(rename::Fail {
                    error,
                    from_dir_wcc: vfs::WccData { before: None, after: None },
                    to_dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let to_dir_path = match self.path_for_handle(&args.to.dir).await {
            Ok(path) => path,
            Err(error) => {
                return Err(rename::Fail {
                    error,
                    from_dir_wcc: vfs::WccData { before: None, after: None },
                    to_dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let from_before_meta = self.metadata(&from_dir_path).await.ok();
        let to_before_meta = self.metadata(&to_dir_path).await.ok();
        let from_before = from_before_meta.as_ref().map(Self::wcc_attr_from_statx);
        let to_before = to_before_meta.as_ref().map(Self::wcc_attr_from_statx);
        let from_before_after = from_before_meta.as_ref().map(Self::attr_from_statx);
        let to_before_after = to_before_meta.as_ref().map(Self::attr_from_statx);

        let mut from_path = from_dir_path.clone();
        from_path.push(args.from.name.as_str());
        let mut to_path = to_dir_path.clone();
        to_path.push(args.to.name.as_str());

        if from_path == to_path {
            return Ok(rename::Success {
                from_dir_wcc: vfs::WccData { before: from_before, after: from_before_after },
                to_dir_wcc: vfs::WccData { before: to_before, after: to_before_after },
            });
        }

        let from_meta = match self.metadata(&from_path).await {
            Ok(meta) => meta,
            Err(error) => {
                return Err(rename::Fail {
                    error,
                    from_dir_wcc: vfs::WccData { before: from_before, after: from_before_after },
                    to_dir_wcc: vfs::WccData { before: to_before, after: to_before_after },
                });
            }
        };

        if let Ok(target_meta) = self.metadata(&to_path).await {
            let from_is_dir =
                matches!(Self::attr_from_statx(&from_meta).file_type, file::Type::Directory);
            let target_is_dir =
                matches!(Self::attr_from_statx(&target_meta).file_type, file::Type::Directory);
            let compatible = from_is_dir == target_is_dir;
            if !compatible {
                return Err(rename::Fail {
                    error: vfs::Error::Exist,
                    from_dir_wcc: vfs::WccData { before: from_before, after: from_before_after },
                    to_dir_wcc: vfs::WccData { before: to_before, after: to_before_after },
                });
            }
            if target_is_dir {
                if let Ok(mut iter) = std::fs::read_dir(&to_path) {
                    if iter.next().is_some() {
                        return Err(rename::Fail {
                            error: vfs::Error::Exist,
                            from_dir_wcc: vfs::WccData {
                                before: from_before,
                                after: from_before_after,
                            },
                            to_dir_wcc: vfs::WccData { before: to_before, after: to_before_after },
                        });
                    }
                }
            }
            self.remove_cached_path(&to_path).await;
        }

        if let Err(error) = std::fs::rename(&from_path, &to_path) {
            return Err(rename::Fail {
                error: Self::io_error_to_vfs(&error),
                from_dir_wcc: self.wcc_data(&from_dir_path, from_before).await,
                to_dir_wcc: self.wcc_data(&to_dir_path, to_before).await,
            });
        }

        if let Err(error) = self.rename_cached_path(&from_path, &to_path).await {
            return Err(rename::Fail {
                error,
                from_dir_wcc: self.wcc_data(&from_dir_path, from_before).await,
                to_dir_wcc: self.wcc_data(&to_dir_path, to_before).await,
            });
        }

        Ok(rename::Success {
            from_dir_wcc: self.wcc_data(&from_dir_path, from_before).await,
            to_dir_wcc: self.wcc_data(&to_dir_path, to_before).await,
        })
    }
}
