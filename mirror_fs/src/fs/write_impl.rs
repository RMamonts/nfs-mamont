use libc;
use nfs_mamont::vfs::{self, write};

use super::MirrorFS;

impl write::Write for MirrorFS {
    async fn write(&self, args: write::Args) -> Result<write::Success, write::Fail> {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(write::Fail {
                    error,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };

        let before_meta = self.metadata(&path).await.ok();
        let before = before_meta.as_ref().map(Self::wcc_attr_from_statx);
        if let Some(attr) = before_meta.as_ref().map(Self::attr_from_statx) {
            if let Err(error) = Self::validate_regular(&attr) {
                return Err(write::Fail { error, wcc_data: self.wcc_data(&path, before).await });
            }
        }

        if let Some(uring) = self.uring_executor() {
            let fd = match self.open_fd_uring(&path, libc::O_WRONLY | libc::O_CLOEXEC, 0).await {
                Ok(fd) => fd,
                Err(error) => {
                    return Err(write::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: self.wcc_data(&path, before).await,
                    });
                }
            };
            for data in &args.data {
                if let Err(error) = self.write_all_uring(fd, args.offset, data).await
                {
                    return Err(write::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: self.wcc_data(&path, before).await,
                    });
                }
            }
            let sync_result = match args.stable {
                write::StableHow::Unstable => Ok(()),
                write::StableHow::DataSync => uring.fsync(fd, true).await,
                write::StableHow::FileSync => uring.fsync(fd, false).await,
            };
            if let Err(error) = sync_result {
                return Err(write::Fail {
                    error: Self::io_error_to_vfs(&error),
                    wcc_data: self.wcc_data(&path, before).await,
                });
            }
        } else {
            return Err(write::Fail {
                error: vfs::Error::IO,
                wcc_data: self.wcc_data(&path, before).await,
            });
        }

        Ok(write::Success {
            file_wcc: self.wcc_data(&path, before).await,
            count: args.size,
            commited: args.stable,
            verifier: self.write_verifier(),
        })
    }
}
