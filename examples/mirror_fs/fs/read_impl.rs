use tokio::fs::File;

use nfs_mamont::vfs::read;
use nfs_mamont::Slice;

use super::MirrorFS;

impl read::Read for MirrorFS {
    async fn read(&self, args: read::Args, data: Slice) -> Result<read::Success, read::Fail> {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(read::Fail { error, file_attr: None });
            }
        };
        let file = match File::open(&path).await {
            Ok(file) => file,
            Err(error) => {
                return Err(read::Fail { error: Self::io_error_to_vfs(&error), file_attr: None });
            }
        };

        let meta = match file.metadata().await {
            Ok(meta) => meta,
            Err(error) => {
                return Err(read::Fail { error: Self::io_error_to_vfs(&error), file_attr: None });
            }
        };
        let attr = Self::attr_from_metadata(&meta);
        if let Err(error) = Self::validate_regular(&attr) {
            return Err(read::Fail { error, file_attr: Some(attr) });
        }

        let file_len = meta.len();
        let start = args.offset.min(file_len);
        let end = args.offset.saturating_add(args.count as u64).min(file_len);
        let read_count = end.saturating_sub(start) as usize;
        let payload = if read_count == 0 {
            read::Payload::Slice(data)
        } else {
            read::Payload::SendFile(read::SendFile {
                file: file.into_std().await,
                offset: start,
                count: read_count,
            })
        };

        Ok(read::Success {
            head: read::SuccessPartial {
                file_attr: Some(attr),
                count: read_count as u32,
                eof: start.saturating_add(read_count as u64) >= file_len,
            },
            data: payload,
        })
    }

    fn supports_sendfile(&self) -> bool {
        true
    }
}
