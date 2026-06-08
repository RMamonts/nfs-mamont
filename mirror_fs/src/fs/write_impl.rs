use libc;
use nfs_mamont::vfs::{self, write};

use super::MirrorFS;

impl write::Write for MirrorFS {
    async fn write(&self, mut args: write::Args) -> Result<write::Success, write::Fail> {
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

            let (buffers, alloc_state) = args.data.take_buffers();
            let do_fsync = match args.stable {
                write::StableHow::Unstable => None,
                write::StableHow::DataSync => Some(true),
                write::StableHow::FileSync => Some(false),
            };

            if let Err(error) =
                uring.write_chain(fd.as_raw_fd(), args.offset, buffers, alloc_state, do_fsync).await
            {
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
            committed: args.stable,
            verifier: self.write_verifier(),
        })
    }
}
