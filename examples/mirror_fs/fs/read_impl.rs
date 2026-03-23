use async_trait::async_trait;
use nfs_mamont::allocator::multilevel::slice::MultiSlice;
use nfs_mamont::vfs::read;
use std::io::SeekFrom;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::MirrorFS;

#[async_trait]
impl read::Read for MirrorFS {
    async fn read(
        &self,
        args: read::Args,
        mut data: MultiSlice,
    ) -> Result<read::Success, read::Fail> {
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

        let mut file = match File::open(&path).await {
            Ok(file) => file,
            Err(error) => {
                return Err(read::Fail {
                    error: Self::io_error_to_vfs(&error),
                    file_attr: Some(attr),
                });
            }
        };

        let file_len = meta.len();
        let start = args.offset.min(file_len);
        let end = args.offset.saturating_add(args.count as u64).min(file_len);
        let requested = end.saturating_sub(start) as usize;
        let mut remaining = requested;
        let mut read_count = 0usize;
        if let Err(error) = file.seek(SeekFrom::Start(start)).await {
            return Err(read::Fail { error: Self::io_error_to_vfs(&error), file_attr: Some(attr) });
        }

        if remaining > 0 {
            for chunk in data.iter_mut() {
                if remaining == 0 {
                    break;
                }

                let to_read = chunk.len().min(remaining);
                let mut chunk_offset = 0usize;

                while chunk_offset < to_read {
                    match file.read(&mut chunk[chunk_offset..to_read]).await {
                        Ok(0) => {
                            remaining = 0;
                            break;
                        }
                        Ok(bytes) => {
                            chunk_offset += bytes;
                            read_count += bytes;
                        }
                        Err(error) => {
                            return Err(read::Fail {
                                error: Self::io_error_to_vfs(&error),
                                file_attr: Some(attr),
                            });
                        }
                    }
                }

                if chunk_offset < to_read {
                    break;
                }

                remaining -= to_read;
            }
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
