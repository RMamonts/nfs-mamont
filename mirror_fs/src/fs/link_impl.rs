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
        let file_attr = self.file_attr(&file_path).await;
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
        let before =
            self.metadata(&dir_path).await.ok().map(|meta| Self::wcc_attr_from_statx(&meta));
        let mut target_path = dir_path.clone();
        target_path.push(args.link.name.as_str());
        if let Err(error) = fs::hard_link(&file_path, &target_path).await {
            return Err(link::Fail {
                error: Self::io_error_to_vfs(&error),
                file_attr,
                dir_wcc: self.wcc_data(&dir_path, before).await,
            });
        }
        let _ = self.handle_for_path(&target_path).await;

        Ok(link::Success {
            file_attr: self.file_attr(&file_path).await,
            dir_wcc: self.wcc_data(&dir_path, before).await,
        })
    }
}
