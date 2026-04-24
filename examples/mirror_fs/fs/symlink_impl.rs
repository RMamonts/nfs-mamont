use nfs_mamont::vfs::{self, symlink};

use super::MirrorFS;

impl symlink::Symlink for MirrorFS {
    async fn symlink(&self, args: symlink::Args) -> Result<symlink::Success, symlink::Fail> {
        let _ = args;
        Err(symlink::Fail {
            error: vfs::Error::NotSupported,
            dir_wcc: vfs::WccData { before: None, after: None },
        })
    }
}
