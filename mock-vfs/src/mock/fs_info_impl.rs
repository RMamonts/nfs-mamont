use nfs_mamont::vfs::file::Time;
use nfs_mamont::vfs::fs_info;

use super::MockVfs;

impl fs_info::FsInfo for MockVfs {
    async fn fs_info(
        &self,
        _args: fs_info::Args,
    ) -> Result<fs_info::Success, fs_info::Fail> {
        Ok(fs_info::Success {
            root_attr: Some(self.config.dir_attr.clone()),
            read_max: 1048576,
            read_pref: 65536,
            read_mult: 4096,
            write_max: 1048576,
            write_pref: 65536,
            write_mult: 4096,
            read_dir_pref: 8192,
            max_file_size: u64::MAX,
            time_delta: Time { seconds: 0, nanos: 1 },
            properties: fs_info::Properties::from_wire(fs_info::Properties::ALL),
        })
    }
}
