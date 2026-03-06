use async_trait::async_trait;

use nfs_mamont::vfs::fs_stat;

use super::*;

#[async_trait]
impl fs_stat::FsStat for MirrorFS {
    async fn fs_stat(&self, args: fs_stat::Args) -> fs_stat::Result {
        let path = match self.path_for_handle(&args.root).await {
            Ok(path) => path,
            Err(error) => return Err(fs_stat::Fail { error, root_attr: None }),
        };
        Ok(fs_stat::Success {
            root_attr: Self::file_attr(&path),
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
