use nfs_mamont::vfs::{self, file, link};

use super::MirrorFS;

impl link::Link for MirrorFS {
    async fn link(&self, args: link::Args) -> Result<link::Success, link::Fail> {
        let _ = args;
        Err(link::Fail {
            error: vfs::Error::NotSupported,
            file_attr: None,
            dir_wcc: vfs::WccData { before: None, after: None },
        })
    }
}
