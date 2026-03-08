use async_trait::async_trait;
use nfs_mamont::vfs::get_attr;
use tracing::{error, info};

use super::*;

#[async_trait]
impl get_attr::GetAttr for MirrorFS {
    async fn get_attr(&self, args: get_attr::Args) -> get_attr::Result {
        info!("get_attr handle={:02x?}", args.file.0);
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => {
                info!("get_attr path={}", path.display());
                path
            }
            Err(error) => {
                error!("get_attr rejected error={error:?}");
                return Err(get_attr::Fail { error });
            }
        };
        match Self::metadata(&path) {
            Ok(meta) => Ok(get_attr::Success { object: Self::attr_from_metadata(&meta) }),
            Err(error) => {
                error!("get_attr failed path={} error={error:?}", path.display());
                Err(get_attr::Fail { error })
            }
        }
    }
}
