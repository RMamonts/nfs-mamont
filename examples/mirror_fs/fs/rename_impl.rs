use async_trait::async_trait;
use std::path::Path;
use tokio::fs;

use nfs_mamont::vfs::{self, rename};

use super::MirrorFS;

#[async_trait]
impl rename::Rename for MirrorFS {
    async fn rename(&self, from: &Path, to: &Path) -> Result<rename::Success, rename::Fail> {
        let from_dir_path = match from.parent() {
            Some(parent) if parent.is_dir() => parent,
            _ => {
                return Err(rename::Fail {
                    error: vfs::Error::BadType,
                    from_dir_wcc: vfs::WccData { before: None, after: None },
                    to_dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };

        let to_dir_path = match to.parent() {
            Some(parent) if parent.is_dir() => parent,
            _ => {
                return Err(rename::Fail {
                    error: vfs::Error::BadType,
                    from_dir_wcc: vfs::WccData { before: None, after: None },
                    to_dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let from_before_meta = std::fs::symlink_metadata(from_dir_path).ok();
        let to_before_meta = std::fs::symlink_metadata(to_dir_path).ok();
        let from_before = from_before_meta.as_ref().map(Self::wcc_attr_from_metadata);
        let to_before = to_before_meta.as_ref().map(Self::wcc_attr_from_metadata);
        let from_before_after = from_before_meta.as_ref().map(Self::attr_from_metadata);
        let to_before_after = to_before_meta.as_ref().map(Self::attr_from_metadata);

        if from == to {
            return Ok(rename::Success {
                from_dir_wcc: vfs::WccData { before: from_before, after: from_before_after },
                to_dir_wcc: vfs::WccData { before: to_before, after: to_before_after },
            });
        }

        let from_meta = match Self::metadata(from) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(rename::Fail {
                    error,
                    from_dir_wcc: vfs::WccData { before: from_before, after: from_before_after },
                    to_dir_wcc: vfs::WccData { before: to_before, after: to_before_after },
                });
            }
        };

        if let Ok(target_meta) = Self::metadata(to) {
            let compatible = from_meta.is_dir() == target_meta.is_dir();
            if !compatible {
                return Err(rename::Fail {
                    error: vfs::Error::Exist,
                    from_dir_wcc: vfs::WccData { before: from_before, after: from_before_after },
                    to_dir_wcc: vfs::WccData { before: to_before, after: to_before_after },
                });
            }
            if target_meta.is_dir() {
                if let Ok(mut iter) = std::fs::read_dir(to) {
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
        }

        if let Err(error) = fs::rename(from, to).await {
            return Err(rename::Fail {
                error: Self::io_error_to_vfs(&error),
                from_dir_wcc: Self::wcc_data(from_dir_path, from_before),
                to_dir_wcc: Self::wcc_data(to_dir_path, to_before),
            });
        }

        Ok(rename::Success {
            from_dir_wcc: Self::wcc_data(from_dir_path, from_before),
            to_dir_wcc: Self::wcc_data(to_dir_path, to_before),
        })
    }
}
