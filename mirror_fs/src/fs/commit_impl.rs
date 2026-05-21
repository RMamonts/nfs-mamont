use libc;
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
        let before_meta = self.metadata(&path).await.ok();
        let before = before_meta.as_ref().map(Self::wcc_attr_from_statx);
        if let Some(attr) = before_meta.as_ref().map(Self::attr_from_statx) {
            if let Err(error) = Self::validate_regular(&attr) {
                return Err(commit::Fail { error, file_wcc: self.wcc_data(&path, before).await });
            }
        }

        if let Some(uring) = self.uring_executor() {
            let fd = match self.open_fd_uring(&path, libc::O_WRONLY | libc::O_CLOEXEC, 0).await {
                Ok(fd) => fd,
                Err(error) => {
                    return Err(commit::Fail {
                        error: Self::io_error_to_vfs(&error),
                        file_wcc: self.wcc_data(&path, before).await,
                    });
                }
            };
            if let Err(error) = uring.fsync(fd, false).await {
                return Err(commit::Fail {
                    error: Self::io_error_to_vfs(&error),
                    file_wcc: self.wcc_data(&path, before).await,
                });
            }
        } else {
            return Err(commit::Fail {
                error: vfs::Error::IO,
                file_wcc: self.wcc_data(&path, before).await,
            });
        }

        Ok(commit::Success {
            file_wcc: self.wcc_data(&path, before).await,
            verifier: self.write_verifier(),
        })
    }
}
