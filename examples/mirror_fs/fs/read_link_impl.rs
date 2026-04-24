use nfs_mamont::vfs::{self, file, read_link};

use super::MirrorFS;

impl read_link::ReadLink for MirrorFS {
    async fn read_link(
        &self,
        args: read_link::Args,
    ) -> Result<read_link::Success, read_link::Fail> {
        let _ = args;
        Err(read_link::Fail { error: vfs::Error::NotSupported, symlink_attr: None })
    }
}
