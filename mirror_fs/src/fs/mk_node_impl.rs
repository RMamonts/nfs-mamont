use nfs_mamont::vfs::{self, mk_node};

use super::MirrorFS;

impl mk_node::MkNode for MirrorFS {
    async fn mk_node(&self, _args: mk_node::Args) -> Result<mk_node::Success, mk_node::Fail> {
        Err(mk_node::Fail {
            error: vfs::Error::NotSupported,
            dir_wcc: vfs::WccData { before: None, after: None },
        })
    }
}
