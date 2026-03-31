use tokio::fs;

use nfs_mamont::vfs::{self, file, link};

use super::MirrorFS;

impl link::Link for MirrorFS {
    async fn link(&self, args: link::Args) -> Result<link::Success, link::Fail> {
        if let Err(error) = Self::ensure_name_allowed(&args.link.name) {
            return Err(link::Fail {
                error,
                file_attr: None,
                dir_wcc: vfs::WccData { before: None, after: None },
            });
        }

        let (file_export_id, file_path) = match self.path_for_handle_with_export(&args.file).await {
            Ok(value) => value,
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

        let (dir_export_id, dir_path) = match self.path_for_handle_with_export(&args.link.dir).await
        {
            Ok(value) => value,
            Err(error) => {
                return Err(link::Fail {
                    error,
                    file_attr,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        if file_export_id != dir_export_id {
            return Err(link::Fail {
                error: vfs::Error::XDev,
                file_attr,
                dir_wcc: vfs::WccData { before: None, after: None },
            });
        }
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
        let _ = self.ensure_handle_for_path(dir_export_id, &target_path).await;

        self.invalidate_attr_cache_path(&file_path).await;
        self.invalidate_attr_cache_path(&target_path).await;
        self.invalidate_attr_cache_path(&dir_path).await;

        Ok(link::Success {
            file_attr: Self::file_attr(&file_path),
            dir_wcc: Self::wcc_data(&dir_path, before),
        })
    }
}
