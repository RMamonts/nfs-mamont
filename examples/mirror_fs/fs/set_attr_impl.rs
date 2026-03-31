use async_trait::async_trait;
use std::path::Path;

use nfs_mamont::vfs::{self, set_attr};

use super::MirrorFS;

#[async_trait]
impl set_attr::SetAttr for MirrorFS {
    async fn set_attr(
        &self,
        args: set_attr::Args,
        path: &Path,
    ) -> Result<set_attr::Success, set_attr::Fail> {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(set_attr::Fail {
                    error,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };
        let meta = match Self::metadata(&path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(set_attr::Fail {
                    error,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = Some(Self::wcc_attr_from_metadata(&meta));
        let current_attr = Self::attr_from_metadata(&meta);

        if let Some(guard) = args.guard {
            if !Self::same_time(current_attr.ctime, guard.ctime) {
                return Err(set_attr::Fail {
                    error: vfs::Error::NotSync,
                    wcc_data: vfs::WccData { before, after: Some(current_attr) },
                });
            }
        }

        if let Err(error) = Self::apply_set_attr(&path, &args.new_attr) {
            return Err(set_attr::Fail { error, wcc_data: Self::wcc_data(&path, before) });
        }

        Ok(set_attr::Success { wcc_data: Self::wcc_data(&path, before) })
    }
}
