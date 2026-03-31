use super::MirrorFS;
use async_trait::async_trait;
use nfs_mamont::vfs::write;
use nfs_mamont::vfs::write::StableHow;
use nfs_mamont::Slice;
use std::io::SeekFrom;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

#[async_trait]
impl write::Write for MirrorFS {
    async fn write(
        &self,
        path: &Path,
        offset: u64,
        size: u32,
        stable: StableHow,
        data: Slice,
    ) -> Result<write::Success, write::Fail> {
        let before_meta = std::fs::symlink_metadata(&path).ok();
        let before = before_meta.as_ref().map(Self::wcc_attr_from_metadata);
        if let Some(attr) = before_meta.as_ref().map(Self::attr_from_metadata) {
            if let Err(error) = Self::validate_regular(&attr) {
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

        let data = Self::collect_slice_bytes(&data, size);
        if let Err(error) = file.seek(SeekFrom::Start(offset)).await {
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
        let sync_result = match stable {
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
            commited: stable,
            verifier: self.write_verifier(),
        })
    }
}
