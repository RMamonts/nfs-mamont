use nfs_mamont::vfs::{self, create, mk_dir, mk_node};

use crate::fs::DEFAULT_SET_ATTR;

use super::MirrorFS;

impl mk_node::MkNode for MirrorFS {
    async fn mk_node(&self, args: mk_node::Args) -> Result<mk_node::Success, mk_node::Fail> {
        let _ = args;
        Err(mk_node::Fail {
            error: vfs::Error::NotSupported,
            dir_wcc: vfs::WccData { before: None, after: None },
        })
    }
}
