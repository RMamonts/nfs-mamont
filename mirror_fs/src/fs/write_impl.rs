use std::io::SeekFrom;
use std::os::unix::io::{AsRawFd, FromRawFd};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

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

        // let data = Self::collect_slice_bytes(&args.data, args.size);
        if self.uring.is_some() {
            let fd = match self.open_fd_uring(&path, libc::O_WRONLY | libc::O_CLOEXEC, 0).await {
                Ok(fd) => fd,
                Err(error) => {
                    return Err(write::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: self.wcc_data(&path, before).await,
                    });
                }
            };
            let file = unsafe { std::fs::File::from_raw_fd(fd) };
            for data in &args.data {
                if let Err(error) = self.write_all_uring(file.as_raw_fd(), args.offset, &data).await
                {
                    drop(file);
                    return Err(write::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: self.wcc_data(&path, before).await,
                    });
                }
            }
            let sync_result = match args.stable {
                write::StableHow::Unstable => Ok(()),
                write::StableHow::DataSync => self.uring.as_ref().unwrap().fsync(fd, true).await,
                write::StableHow::FileSync => self.uring.as_ref().unwrap().fsync(fd, false).await,
            };
            drop(file);
            if let Err(error) = sync_result {
                return Err(write::Fail {
                    error: Self::io_error_to_vfs(&error),
                    wcc_data: self.wcc_data(&path, before).await,
                });
            }
        } else {
            let mut file = match OpenOptions::new().write(true).truncate(false).open(&path).await {
                Ok(file) => file,
                Err(error) => {
                    return Err(write::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: self.wcc_data(&path, before).await,
                    });
                }
            };
            if let Err(error) = file.seek(SeekFrom::Start(args.offset)).await {
                return Err(write::Fail {
                    error: Self::io_error_to_vfs(&error),
                    wcc_data: self.wcc_data(&path, before).await,
                });
            }
            for data in &args.data {
                if let Err(error) = file.write_all(&data).await {
                    return Err(write::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: self.wcc_data(&path, before).await,
                    });
                }
            }
            let sync_result = match args.stable {
                write::StableHow::Unstable => Ok(()),
                write::StableHow::DataSync => self.fsync_file(&file, true).await,
                write::StableHow::FileSync => self.fsync_file(&file, false).await,
            };
            if let Err(error) = sync_result {
                return Err(write::Fail {
                    error: Self::io_error_to_vfs(&error),
                    wcc_data: self.wcc_data(&path, before).await,
                });
            }
        }

        Ok(write::Success {
            file_wcc: self.wcc_data(&path, before).await,
            count: args.size as u32,
            commited: args.stable,
            verifier: self.write_verifier(),
        })
    }
}
