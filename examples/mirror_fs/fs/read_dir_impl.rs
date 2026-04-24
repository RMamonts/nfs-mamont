use nfs_mamont::vfs::{self, read_dir};

use super::MirrorFS;

impl read_dir::ReadDir for MirrorFS {
    async fn read_dir(&self, args: read_dir::Args) -> Result<read_dir::Success, read_dir::Fail> {
        let _ = args;
        Err(read_dir::Fail { error: vfs::Error::NotSupported, dir_attr: None })
    }
}
