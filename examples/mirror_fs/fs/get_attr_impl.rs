use async_trait::async_trait;
use std::path::Path;

use nfs_mamont::vfs::get_attr;

use super::MirrorFS;

#[async_trait]
impl get_attr::GetAttr for MirrorFS {
    async fn get_attr(&self, path: &Path) -> Result<get_attr::Success, get_attr::Fail> {
        match Self::metadata(path) {
            Ok(meta) => Ok(get_attr::Success { object: Self::attr_from_metadata(&meta) }),
            Err(error) => Err(get_attr::Fail { error }),
        }
    }
}
