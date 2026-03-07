use async_trait::async_trait;
use std::io::SeekFrom;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

use nfs_mamont::vfs::write;

use super::*;

#[async_trait]
impl write::Write for MirrorFS {
    async fn write(&self, args: write::Args) -> write::Result {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(write::Fail {
                    error,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };

        let before =
            std::fs::symlink_metadata(&path).ok().map(|meta| Self::wcc_attr_from_metadata(&meta));
        if let Some(ref attr) = Self::file_attr(&path) {
            if let Err(error) = Self::validate_regular(attr) {
                return Err(write::Fail { error, wcc_data: Self::wcc_data(&path, before) });
            }
        }

        let mut file = match OpenOptions::new().write(true).truncate(false).open(&path).await {
            Ok(file) => file,
            Err(error) => {
                return Err(write::Fail {
                    error: Self::io_error_to_vfs(&error),
                    wcc_data: Self::wcc_data(&path, before),
                });
            }
        };

        let data = Self::collect_slice_bytes(&args.data, args.size);
        if let Err(error) = file.seek(SeekFrom::Start(args.offset)).await {
            return Err(write::Fail {
                error: Self::io_error_to_vfs(&error),
                wcc_data: Self::wcc_data(&path, before),
            });
        }
        if let Err(error) = file.write_all(&data).await {
            return Err(write::Fail {
                error: Self::io_error_to_vfs(&error),
                wcc_data: Self::wcc_data(&path, before),
            });
        }
        let sync_result = match args.stable {
            write::StableHow::Unstable => Ok(()),
            write::StableHow::DataSync => file.sync_data().await,
            write::StableHow::FileSync => file.sync_all().await,
        };
        if let Err(error) = sync_result {
            return Err(write::Fail {
                error: Self::io_error_to_vfs(&error),
                wcc_data: Self::wcc_data(&path, before),
            });
        }

        Ok(write::Success {
            file_wcc: Self::wcc_data(&path, before),
            count: data.len() as u32,
            commited: args.stable,
            verifier: self.write_verifier(),
        })
    }
}
