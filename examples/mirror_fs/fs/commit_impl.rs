use async_trait::async_trait;
use tokio::fs::OpenOptions;

use nfs_mamont::vfs::commit;

use super::*;

#[async_trait]
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
        let before =
            std::fs::symlink_metadata(&path).ok().map(|meta| Self::wcc_attr_from_metadata(&meta));
        if let Some(ref attr) = Self::file_attr(&path) {
            if let Err(error) = Self::validate_regular(attr) {
                return Err(commit::Fail { error, file_wcc: Self::wcc_data(&path, before) });
            }
        }

        let file = match OpenOptions::new().write(true).open(&path).await {
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
