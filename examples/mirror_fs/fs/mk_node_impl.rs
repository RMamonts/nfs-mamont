use async_trait::async_trait;
use std::path::Path;

use crate::fs::DEFAULT_SET_ATTR;
use nfs_mamont::vfs::{self, create, mk_dir, mk_node};

use super::MirrorFS;

#[async_trait]
impl mk_node::MkNode for MirrorFS {
    async fn mk_node(
        &self,
        path: &Path,
        what: mk_node::What,
    ) -> Result<mk_node::Success, mk_node::Fail> {
        match what {
            mk_node::What::Regular => {
                match create::Create::create(self, path, create::How::Unchecked(DEFAULT_SET_ATTR))
                    .await
                {
                    Ok(success) => Ok(mk_node::Success {
                        file: success.file,
                        attr: success.attr,
                        wcc_data: success.wcc_data,
                    }),
                    Err(fail) => Err(mk_node::Fail { error: fail.error, dir_wcc: fail.wcc_data }),
                }
            }
            mk_node::What::Directory => {
                match mk_dir::MkDir::mk_dir(self, path, DEFAULT_SET_ATTR).await {
                    Ok(success) => Ok(mk_node::Success {
                        file: success.file,
                        attr: success.attr,
                        wcc_data: success.wcc_data,
                    }),
                    Err(fail) => Err(mk_node::Fail { error: fail.error, dir_wcc: fail.dir_wcc }),
                }
            }
            mk_node::What::SymbolicLink => Err(mk_node::Fail {
                error: vfs::Error::BadType,
                dir_wcc: vfs::WccData { before: None, after: None },
            }),
            mk_node::What::Char(_, _)
            | mk_node::What::Block(_, _)
            | mk_node::What::Socket(_)
            | mk_node::What::Fifo(_) => Err(mk_node::Fail {
                error: vfs::Error::NotSupported,
                dir_wcc: vfs::WccData { before: None, after: None },
            }),
        }
    }
}
