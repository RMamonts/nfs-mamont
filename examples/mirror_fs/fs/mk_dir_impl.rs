use nfs_mamont::vfs::{self, mk_dir};

use super::MirrorFS;

impl mk_dir::MkDir for MirrorFS {
    async fn mk_dir(&self, args: mk_dir::Args) -> Result<mk_dir::Success, mk_dir::Fail> {
        let _ = args;
        Err(mk_dir::Fail {
            error: vfs::Error::NotSupported,
            dir_wcc: vfs::WccData { before: None, after: None },
        })
    }
}
