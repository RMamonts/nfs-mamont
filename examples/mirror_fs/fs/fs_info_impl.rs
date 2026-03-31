use async_trait::async_trait;
use std::path::Path;

use nfs_mamont::vfs::file;
use nfs_mamont::vfs::fs_info;

use super::{MirrorFS, READ_DIR_PREF, READ_WRITE_MAX};

#[async_trait]
impl fs_info::FsInfo for MirrorFS {
    async fn fs_info(
        &self,
        args: fs_info::Args,
        root: &Path,
    ) -> Result<fs_info::Success, fs_info::Fail> {
        let path = match self.path_for_handle(&args.root).await {
            Ok(path) => path,
            Err(error) => return Err(fs_info::Fail { error, root_attr: None }),
        };
        Ok(fs_info::Success {
            root_attr: Self::file_attr(&path),
            read_max: READ_WRITE_MAX,
            read_pref: READ_WRITE_MAX,
            read_mult: 1,
            write_max: READ_WRITE_MAX,
            write_pref: READ_WRITE_MAX,
            write_mult: 1,
            read_dir_pref: READ_DIR_PREF,
            max_file_size: u64::MAX,
            time_delta: file::Time { seconds: 0, nanos: 1 },
            properties: fs_info::Properties::from_wire(
                fs_info::Properties::LINK
                    | fs_info::Properties::SYMLINK
                    | fs_info::Properties::HOMOGENEOUS
                    | fs_info::Properties::CANSETTIME,
            ),
        })
    }
}
