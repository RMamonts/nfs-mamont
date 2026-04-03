use async_trait::async_trait;
use std::path::Path;

use super::MirrorFS;
use nfs_mamont::vfs::set_attr::NewAttr;
use nfs_mamont::vfs::{self, symlink};

#[async_trait]
impl symlink::Symlink for MirrorFS {
    async fn symlink(
        &self,
        path: &Path,
        obj: &Path,
        _new_attr: NewAttr,
    ) -> Result<symlink::Success, symlink::Fail> {
        let dir_path = match path.parent() {
            Some(parent) if parent.is_dir() => parent,
            _ => {
                return Err(symlink::Fail {
                    error: vfs::Error::BadType,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = std::fs::symlink_metadata(dir_path)
            .ok()
            .map(|meta| Self::wcc_attr_from_metadata(&meta));

        match std::os::unix::fs::symlink(obj, path) {
            Ok(()) => {}
            Err(error) => {
                return Err(symlink::Fail {
                    error: Self::io_error_to_vfs(&error),
                    dir_wcc: Self::wcc_data(dir_path, before),
                });
            }
        }

        let attr = match Self::metadata(path) {
            Ok(meta) => Self::attr_from_metadata(&meta),
            Err(error) => {
                return Err(symlink::Fail { error, dir_wcc: Self::wcc_data(dir_path, before) })
            }
        };

        Ok(symlink::Success {
            file: None,
            attr: Some(attr),
            wcc_data: Self::wcc_data(dir_path, before),
        })
    }
}
