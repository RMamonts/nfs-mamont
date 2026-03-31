use async_trait::async_trait;
use std::path::Path;
use tokio::fs;

use nfs_mamont::vfs::{self, file, link};

use super::MirrorFS;

#[async_trait]
impl link::Link for MirrorFS {
    async fn link(
        &self,
        args: link::Args,
        path: &Path,
        object: &Path,
    ) -> Result<link::Success, link::Fail> {
        if let Err(error) = Self::ensure_name_allowed(&args.link.name) {
            return Err(link::Fail {
                error,
                file_attr: None,
                dir_wcc: vfs::WccData { before: None, after: None },
            });
        }

        let file_path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(link::Fail {
                    error,
                    file_attr: None,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let file_attr = Self::file_attr(&file_path);
        if matches!(file_attr.as_ref().map(|attr| attr.file_type), Some(file::Type::Directory)) {
            return Err(link::Fail {
                error: vfs::Error::InvalidArgument,
                file_attr,
                dir_wcc: vfs::WccData { before: None, after: None },
            });
        }

        let dir_path = match self.path_for_handle(&args.link.dir).await {
            Ok(path) => path,
            Err(error) => {
                return Err(link::Fail {
                    error,
                    file_attr,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = std::fs::symlink_metadata(&dir_path)
            .ok()
            .map(|meta| Self::wcc_attr_from_metadata(&meta));
        let mut target_path = dir_path.clone();
        target_path.push(args.link.name.as_str());
        if let Err(error) = fs::hard_link(&file_path, &target_path).await {
            return Err(link::Fail {
                error: Self::io_error_to_vfs(&error),
                file_attr,
                dir_wcc: Self::wcc_data(&dir_path, before),
            });
        }
        let _ = self.ensure_handle_for_path(&target_path).await;

        Ok(link::Success {
            file_attr: Self::file_attr(&file_path),
            dir_wcc: Self::wcc_data(&dir_path, before),
        })
    }
}
