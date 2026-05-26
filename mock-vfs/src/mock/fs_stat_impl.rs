use nfs_mamont::vfs::fs_stat;

use super::MockVfs;

impl fs_stat::FsStat for MockVfs {
    async fn fs_stat(
        &self,
        _args: fs_stat::Args,
    ) -> Result<fs_stat::Success, fs_stat::Fail> {
        Ok(fs_stat::Success {
            root_attr: Some(self.config.dir_attr.clone()),
            total_bytes: self.config.file_size,
            free_bytes: u64::MAX,
            available_bytes: u64::MAX,
            total_files: u64::MAX,
            free_files: u64::MAX,
            available_files: u64::MAX,
            invarsec: 0,
        })
    }
}
