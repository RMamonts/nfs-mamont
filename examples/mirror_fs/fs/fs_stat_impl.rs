use async_trait::async_trait;
use std::path::Path;

use nfs_mamont::vfs::fs_stat;

use super::MirrorFS;

#[async_trait]
impl fs_stat::FsStat for MirrorFS {
    async fn fs_stat(&self, root: &Path) -> Result<fs_stat::Success, fs_stat::Fail> {
        Ok(fs_stat::Success {
            root_attr: Self::file_attr(root),
            total_bytes: 0,
            free_bytes: 0,
            available_bytes: 0,
            total_files: 0,
            free_files: 0,
            available_files: 0,
            invarsec: 0,
        })
    }
}
