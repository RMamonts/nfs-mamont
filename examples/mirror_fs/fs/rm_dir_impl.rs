use async_trait::async_trait;
use std::path::Path;

use nfs_mamont::vfs::{self, rm_dir};

use super::MirrorFS;

#[async_trait]
impl rm_dir::RmDir for MirrorFS {
    async fn rm_dir(&self, path: &Path) -> Result<rm_dir::Success, rm_dir::Fail> {
        let dir_path = match path.parent() {
            Some(parent) if parent.is_dir() => parent,
            _ => {
                return Err(rm_dir::Fail {
                    error: vfs::Error::BadType,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = std::fs::symlink_metadata(dir_path)
            .ok()
            .map(|meta| Self::wcc_attr_from_metadata(&meta));

        let child_meta = match Self::metadata(path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(rm_dir::Fail { error, dir_wcc: Self::wcc_data(dir_path, before) })
            }
        };
        if !child_meta.is_dir() {
            return Err(rm_dir::Fail {
                error: vfs::Error::NotDir,
                dir_wcc: Self::wcc_data(dir_path, before),
            });
        }

        match std::fs::remove_dir(path) {
            Ok(()) => Ok(rm_dir::Success { wcc_data: Self::wcc_data(dir_path, before) }),
            Err(error) => Err(rm_dir::Fail {
                error: Self::io_error_to_vfs(&error),
                dir_wcc: Self::wcc_data(dir_path, before),
            }),
        }
    }
}
