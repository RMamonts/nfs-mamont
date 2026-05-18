use nfs_mamont::vfs::commit;

use crate::async_fs::{self, File};

use super::*;

impl commit::Commit for MirrorFS {
    async fn commit(&self, args: commit::Args) -> Result<commit::Success, commit::Fail> {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(commit::Fail {
                    error,
                    file_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };

        let before_meta = async_fs::symlink_metadata(&path).await.ok();
        let before = before_meta.as_ref().map(Self::wcc_attr_from_metadata);
        if let Some(attr) = before_meta.as_ref().map(Self::attr_from_metadata) {
            if let Err(error) = Self::validate_regular(&attr) {
                return Err(commit::Fail { error, file_wcc: Self::wcc_data(&path, before) });
            }
        }

        let mut file = match File::open_write(&path).await {
            Ok(file) => file,
            Err(error) => {
                return Err(commit::Fail {
                    error: Self::io_error_to_vfs(&error),
                    file_wcc: Self::wcc_data(&path, before),
                });
            }
        };

        if let Err(error) = file.sync_all().await {
            return Err(commit::Fail {
                error: Self::io_error_to_vfs(&error),
                file_wcc: Self::wcc_data(&path, before),
            });
        }

        Ok(commit::Success {
            file_wcc: Self::wcc_data(&path, before),
            verifier: self.write_verifier(),
        })
    }
}