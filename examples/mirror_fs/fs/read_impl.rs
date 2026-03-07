use async_trait::async_trait;
use std::io::SeekFrom;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use nfs_mamont::vfs::read;

use super::*;

#[async_trait]
impl read::Read for MirrorFS {
    async fn read(&self, args: read::Args) -> read::Result {
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
        let count = end.saturating_sub(start) as usize;
        if let Err(error) = file.seek(SeekFrom::Start(start)).await {
            return Err(read::Fail { error: Self::io_error_to_vfs(&error), file_attr: Some(attr) });
        }

        let mut bytes = vec![0u8; count];
        if count > 0 {
            if let Err(error) = file.read_exact(&mut bytes).await {
                return Err(read::Fail {
                    error: Self::io_error_to_vfs(&error),
                    file_attr: Some(attr),
                });
            }
        }

        Ok(read::Success {
            head: read::SuccessPartial {
                file_attr: Some(attr),
                count: count as u32,
                eof: end >= file_len,
            },
            data: Self::slice_from_bytes(bytes),
        })
    }
}
