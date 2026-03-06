use async_trait::async_trait;
use nfs_mamont::vfs::get_attr;

use super::*;

#[async_trait]
impl get_attr::GetAttr for MirrorFS {
    async fn get_attr(&self, args: get_attr::Args) -> get_attr::Result {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => return Err(get_attr::Fail { error }),
        };
        match Self::metadata(&path) {
            Ok(meta) => Ok(get_attr::Success { object: Self::attr_from_metadata(&meta) }),
            Err(error) => Err(get_attr::Fail { error }),
        }
    }
}
