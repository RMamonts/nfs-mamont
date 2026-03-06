use async_trait::async_trait;

use nfs_mamont::vfs::access;

use super::*;

#[async_trait]
impl access::Access for MirrorFS {
    async fn access(&self, args: access::Args) -> access::Result {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => return Err(access::Fail { error, object_attr: None }),
        };
        let attr = match Self::metadata(&path) {
            Ok(meta) => Self::attr_from_metadata(&meta),
            Err(error) => return Err(access::Fail { error, object_attr: None }),
        };
        Ok(access::Success { object_attr: Some(attr), access: args.mask })
    }
}
