use nfs_mamont::vfs::{self, rm_dir};

use super::MirrorFS;

impl rm_dir::RmDir for MirrorFS {
    async fn rm_dir(&self, args: rm_dir::Args) -> Result<rm_dir::Success, rm_dir::Fail> {
        let _ = args;
        Err(rm_dir::Fail {
            error: vfs::Error::NotSupported,
            dir_wcc: vfs::WccData { before: None, after: None },
        })
    }
}
