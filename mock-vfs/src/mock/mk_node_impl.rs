use nfs_mamont::vfs;
use nfs_mamont::vfs::mk_node;

use super::MockVfs;

impl mk_node::MkNode for MockVfs {
    async fn mk_node(&self, args: mk_node::Args) -> Result<mk_node::Success, mk_node::Fail> {
        match args.what {
            mk_node::What::Regular | mk_node::What::Directory => {
                let handle = self.next_handle();
                Ok(mk_node::Success {
                    file: Some(handle),
                    attr: Some(self.file_attr()),
                    wcc_data: self.dir_wcc(),
                })
            }
            _ => Err(mk_node::Fail { error: vfs::Error::NotSupported, dir_wcc: self.dir_wcc() }),
        }
    }
}
