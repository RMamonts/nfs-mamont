use super::*;
use async_trait::async_trait;
use nfs_mamont::vfs::commit;
use nfs_mamont::vfs::commit::{Fail, Success};
use tokio::fs::OpenOptions;

#[async_trait]
impl commit::Commit for MirrorFS {
    async fn commit(&self, path: &Path, _offset: u64, _count: u32) -> Result<Success, Fail> {
        let before_meta = std::fs::symlink_metadata(&path).ok();
        let before = before_meta.as_ref().map(Self::wcc_attr_from_metadata);
        if let Some(attr) = before_meta.as_ref().map(Self::attr_from_metadata) {
            if let Err(error) = Self::validate_regular(&attr) {
                return Err(commit::Fail { error, file_wcc: Self::wcc_data(&path, before) });
            }
        }

        let file = match OpenOptions::new().write(true).open(&path).await {
            Ok(file) => file,
            Err(error) => {
                return Err(commit::Fail {
                    error: Self::io_error_to_vfs(&error),
                    file_wcc: Self::wcc_data(&path, before),
                });
            }
        };
        if let Err(error) = file.sync_all().await {
            return Err(commit::Fail {
                error: Self::io_error_to_vfs(&error),
                file_wcc: Self::wcc_data(&path, before),
            });
        }

        Ok(commit::Success {
            file_wcc: Self::wcc_data(&path, before),
            verifier: self.write_verifier(),
        })
    }
}
