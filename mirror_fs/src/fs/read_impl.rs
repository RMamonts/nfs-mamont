use nfs_mamont::vfs;

use libc;
use nfs_mamont::vfs::read;
use nfs_mamont::Slice;

use super::MirrorFS;

impl read::Read for MirrorFS {
    async fn read(&self, args: read::Args, mut data: Slice) -> Result<read::Success, read::Fail> {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(read::Fail { error, file_attr: None });
            }
        };
        let meta = match self.metadata(&path).await {
            Ok(meta) => meta,
            Err(error) => {
                return Err(read::Fail { error, file_attr: None });
            }
        };
        let attr = Self::attr_from_statx(&meta);
        if let Err(error) = Self::validate_regular(&attr) {
            return Err(read::Fail { error, file_attr: Some(attr) });
        }

        let file_len = meta.size;
        let start = args.offset.min(file_len);
        let end = args.offset.saturating_add(args.count as u64).min(file_len);
        let requested = end.saturating_sub(start) as usize;
        let read_count;

        if self.uring_executor().is_some() {
            let fd = match self.open_fd_uring(&path, libc::O_RDONLY | libc::O_CLOEXEC, 0).await {
                Ok(fd) => fd,
                Err(error) => {
                    return Err(read::Fail {
                        error: Self::io_error_to_vfs(&error),
                        file_attr: Some(attr),
                    });
                }
            };

            let buffer = match self.read_at_uring(fd, start, requested).await {
                Ok(buffer) => buffer,
                Err(error) => {
                    return Err(read::Fail {
                        error: Self::io_error_to_vfs(&error),
                        file_attr: Some(attr),
                    });
                }
            };

            read_count = buffer.len();
            let mut offset = 0usize;
            for chunk in data.iter_mut() {
                if offset >= buffer.len() {
                    break;
                }

                let to_copy = (buffer.len() - offset).min(chunk.len());
                chunk[..to_copy].copy_from_slice(&buffer[offset..offset + to_copy]);
                offset += to_copy;
            }
        } else {
            return Err(read::Fail { error: vfs::Error::IO, file_attr: Some(attr) });
        }

        Ok(read::Success {
            head: read::SuccessPartial {
                file_attr: Some(attr),
                count: read_count as u32,
                eof: start.saturating_add(read_count as u64) >= file_len,
            },
            data,
        })
    }
}
