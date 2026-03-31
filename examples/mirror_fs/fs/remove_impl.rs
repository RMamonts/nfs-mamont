use async_trait::async_trait;
use std::path::Path;
use tokio::fs;

use nfs_mamont::vfs::{self, remove};

use super::MirrorFS;

#[async_trait]
impl remove::Remove for MirrorFS {
    async fn remove(&self, path: &Path) -> Result<remove::Success, remove::Fail> {
        //TODO(make ensure path?)

        let dir_path = match path.parent() {
            Some(parent) if parent.is_dir() => parent,
            _ => {
                return Err(remove::Fail {
                    error: vfs::Error::BadType,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = std::fs::symlink_metadata(&dir_path)
            .ok()
            .map(|meta| Self::wcc_attr_from_metadata(&meta));

        let child_meta = match Self::metadata(path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(remove::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) });
            }
        };
        if child_meta.is_dir() {
            return Err(remove::Fail {
                error: vfs::Error::IsDir,
                dir_wcc: Self::wcc_data(&dir_path, before),
            });
        }

        if let Err(error) = fs::remove_file(path).await {
            return Err(remove::Fail {
                error: Self::io_error_to_vfs(&error),
                dir_wcc: Self::wcc_data(&dir_path, before),
            });
        }
        self.remove_cached_path(path).await;

        Ok(remove::Success { wcc_data: Self::wcc_data(&dir_path, before) })
    }
}
