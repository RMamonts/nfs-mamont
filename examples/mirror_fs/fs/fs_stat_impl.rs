use nfs_mamont::vfs::fs_stat;

use super::MirrorFS;

impl fs_stat::FsStat for MirrorFS {
    async fn fs_stat(&self, args: fs_stat::Args) -> Result<fs_stat::Success, fs_stat::Fail> {
        let path = match self.path_for_handle(&args.root).await {
            Ok(path) => path,
            Err(error) => return Err(fs_stat::Fail { error, root_attr: None }),
        };

        let stat = match nix::sys::statvfs::statvfs(&path) {
            Ok(stat) => stat,
            Err(e) => {
                return Err(fs_stat::Fail {
                    error: Self::io_error_to_vfs(&std::io::Error::from_raw_os_error(e as i32)),
                    root_attr: None,
                });
            }
        };

        let frsize = stat.fragment_size() as u64;

        Ok(fs_stat::Success {
            root_attr: Self::file_attr(&path),
            total_bytes: stat.blocks() as u64 * frsize,
            free_bytes: stat.blocks_free() as u64 * frsize,
            available_bytes: stat.blocks_available() as u64 * frsize,
            total_files: stat.files() as u64,
            free_files: stat.files_free() as u64,
            available_files: stat.files_available() as u64,
            invarsec: 0,
        })
    }
}
