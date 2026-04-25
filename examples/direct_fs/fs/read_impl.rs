use async_trait::async_trait;

use nfs_mamont::vfs::read;
use nfs_mamont::Slice;

use super::MirrorFS;

#[async_trait]
impl read::Read for MirrorFS {
    async fn read(&self, args: read::Args, mut data: Slice) -> Result<read::Success, read::Fail> {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(read::Fail { error, file_attr: None });
            }
        };
        let meta = match Self::metadata(&path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(read::Fail { error, file_attr: None });
            }
        };
        let attr = Self::attr_from_metadata(&meta);
        if let Err(error) = Self::validate_regular(&attr) {
            return Err(read::Fail { error, file_attr: Some(attr) });
        }

        let file_len = meta.len();
        let start = args.offset.min(file_len);
        let end = args.offset.saturating_add(args.count as u64).min(file_len);
        let requested = end.saturating_sub(start) as usize;
        let try_direct = self.should_try_direct_io(start, requested);
        let file = match Self::open_for_read(&path, try_direct) {
            Ok(file) => file,
            Err(error) => {
                return Err(read::Fail {
                    error: Self::io_error_to_vfs(&error),
                    file_attr: Some(attr),
                });
            }
        };

        let mut read_buf = vec![0u8; requested];
        let read_count = match Self::read_at_blocking(&file, start, &mut read_buf) {
            Ok(n) => n,
            Err(error) => {
                if try_direct {
                    let fallback_file = match Self::open_for_read(&path, false) {
                        Ok(file) => file,
                        Err(open_error) => {
                            return Err(read::Fail {
                                error: Self::io_error_to_vfs(&open_error),
                                file_attr: Some(attr),
                            });
                        }
                    };

                    match Self::read_at_blocking(&fallback_file, start, &mut read_buf) {
                        Ok(n) => n,
                        Err(fallback_error) => {
                            return Err(read::Fail {
                                error: Self::io_error_to_vfs(&fallback_error),
                                file_attr: Some(attr),
                            });
                        }
                    }
                } else {
                    return Err(read::Fail {
                        error: Self::io_error_to_vfs(&error),
                        file_attr: Some(attr),
                    });
                }
            }
        };

        let mut copied = 0usize;
        for chunk in data.iter_mut() {
            if copied >= read_count {
                break;
            }
            let to_copy = (read_count - copied).min(chunk.len());
            chunk[..to_copy].copy_from_slice(&read_buf[copied..copied + to_copy]);
            copied += to_copy;
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
