use std::io::SeekFrom;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

use nfs_mamont::vfs::{self, write};

use super::MirrorFS;

impl write::Write for MirrorFS {
    async fn write(&self, args: write::Args) -> Result<write::Success, write::Fail> {
        let mut payload = Vec::with_capacity(args.size as usize);
        let mut remaining_payload = args.size as usize;
        for part in &args.data {
            if remaining_payload == 0 {
                break;
            }

            let take = part.len().min(remaining_payload);
            payload.extend_from_slice(&part[..take]);
            remaining_payload -= take;
        }

        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(write::Fail {
                    error,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };

        let mut file = match OpenOptions::new().write(true).truncate(false).open(&path).await {
            Ok(file) => file,
            Err(error) => {
                return Err(write::Fail {
                    error: Self::io_error_to_vfs(&error),
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };

        let before_meta = match file.metadata().await {
            Ok(meta) => meta,
            Err(error) => {
                return Err(write::Fail {
                    error: Self::io_error_to_vfs(&error),
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = Some(Self::wcc_attr_from_metadata(&before_meta));
        let attr = Self::attr_from_metadata(&before_meta);
        if let Err(error) = Self::validate_regular(&attr) {
            return Err(write::Fail { error, wcc_data: Self::wcc_data(&path, before) });
        }

        if let Err(error) = file.seek(SeekFrom::Start(args.offset)).await {
            return Err(write::Fail {
                error: Self::io_error_to_vfs(&error),
                wcc_data: Self::wcc_data(&path, before),
            });
        }

        let written = payload.len() as u32;
        if !payload.is_empty() {
            if let Err(error) = file.write_all(&payload).await {
                return Err(write::Fail {
                    error: Self::io_error_to_vfs(&error),
                    wcc_data: Self::wcc_data(&path, before),
                });
            }
        }

        match args.stable {
            write::StableHow::Unstable => {
                self.mark_pending_unstable_write(&path).await;
            }
            write::StableHow::DataSync => {
                if let Err(error) = file.sync_data().await {
                    return Err(write::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: Self::wcc_data(&path, before),
                    });
                }
                self.clear_pending_unstable_write(&path).await;
            }
            write::StableHow::FileSync => {
                if let Err(error) = file.sync_all().await {
                    return Err(write::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: Self::wcc_data(&path, before),
                    });
                }
                self.clear_pending_unstable_write(&path).await;
            }
        }

        self.invalidate_read_ahead_path(&path).await;
        self.invalidate_attr_cache_path(&path).await;

        Ok(write::Success {
            file_wcc: Self::wcc_data(&path, before),
            count: written,
            commited: args.stable,
            verifier: self.write_verifier(),
        })
    }
}
