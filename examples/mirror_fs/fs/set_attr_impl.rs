use nfs_mamont::vfs::{self, set_attr};

use super::MirrorFS;

impl set_attr::SetAttr for MirrorFS {
    async fn set_attr(&self, args: set_attr::Args) -> Result<set_attr::Success, set_attr::Fail> {
        let _ = args;
        Err(set_attr::Fail {
            error: vfs::Error::NotSupported,
            wcc_data: vfs::WccData { before: None, after: None },
        })
    }
}
