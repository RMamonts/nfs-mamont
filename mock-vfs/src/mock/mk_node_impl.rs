use nfs_mamont::vfs;
use nfs_mamont::vfs::mk_node;

use super::MockVfs;

impl mk_node::MkNode for MockVfs {
    async fn mk_node(&self, _args: mk_node::Args) -> Result<mk_node::Success, mk_node::Fail> {
        // Mock backend simulates regular files and directories only, which are
        // created via CREATE/MKDIR. Special files (char, block, socket, fifo)
        // are not backed by this virtual filesystem.
        Err(mk_node::Fail { error: vfs::Error::NotSupported, dir_wcc: self.dir_wcc() })
    }
}
