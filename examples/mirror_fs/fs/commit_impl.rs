use tokio::fs::OpenOptions;

use nfs_mamont::vfs::commit;

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

        let file = match OpenOptions::new().write(true).open(&path).await {
            Ok(file) => file,
            Err(error) => {
                return Err(commit::Fail {
                    error: Self::io_error_to_vfs(&error),
                    file_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };

        let before_meta = match file.metadata().await {
            Ok(meta) => meta,
            Err(error) => {
                return Err(commit::Fail {
                    error: Self::io_error_to_vfs(&error),
                    file_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = Some(Self::wcc_attr_from_metadata(&before_meta));
        let attr = Self::attr_from_metadata(&before_meta);
        if let Err(error) = Self::validate_regular(&attr) {
            return Err(commit::Fail { error, file_wcc: Self::wcc_data(&path, before) });
        }

        if let Err(error) = file.sync_all().await {
            return Err(commit::Fail {
                error: Self::io_error_to_vfs(&error),
                file_wcc: Self::wcc_data(&path, before),
            });
        }

        self.clear_pending_unstable_write(&path).await;

        Ok(commit::Success {
            file_wcc: Self::wcc_data(&path, before),
            verifier: self.write_verifier(),
        })
    }
}
