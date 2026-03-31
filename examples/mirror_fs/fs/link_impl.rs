use async_trait::async_trait;
use std::path::Path;
use tokio::fs;

use nfs_mamont::vfs::{self, file, link};

use super::MirrorFS;

#[async_trait]
impl link::Link for MirrorFS {
    async fn link(&self, path: &Path, object: &Path) -> Result<link::Success, link::Fail> {
        //TODO(make ensure path?)
        let file_attr = Self::file_attr(object);
        if matches!(file_attr.as_ref().map(|attr| attr.file_type), Some(file::Type::Directory)) {
            return Err(link::Fail {
                error: vfs::Error::InvalidArgument,
                file_attr,
                dir_wcc: vfs::WccData { before: None, after: None },
            });
        }

        let dir_path = match path.parent() {
            Some(parent) if parent.is_dir() => parent,
            _ => {
                return Err(link::Fail {
                    error: vfs::Error::BadType,
                    file_attr,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };

        let before = std::fs::symlink_metadata(dir_path)
            .ok()
            .map(|meta| Self::wcc_attr_from_metadata(&meta));

        if let Err(error) = fs::hard_link(object, path).await {
            return Err(link::Fail {
                error: Self::io_error_to_vfs(&error),
                file_attr,
                dir_wcc: Self::wcc_data(dir_path, before),
            });
        }

        Ok(link::Success {
            file_attr: Self::file_attr(object),
            dir_wcc: Self::wcc_data(dir_path, before),
        })
    }
}
