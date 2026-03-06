use async_trait::async_trait;
use nfs_mamont::vfs::lookup;

use super::*;

#[async_trait]
impl lookup::Lookup for MirrorFS {
    async fn lookup(&self, args: lookup::Args) -> lookup::Result {
        let parent_path = match self.path_for_handle(&args.parent).await {
            Ok(path) => path,
            Err(error) => {
                return Err(lookup::Fail { error, dir_attr: None });
            }
        };
        let parent_meta = match Self::metadata(&parent_path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(lookup::Fail { error, dir_attr: None });
            }
        };
        let parent_attr = Self::attr_from_metadata(&parent_meta);
        if let Err(error) = Self::validate_directory(&parent_attr) {
            return Err(lookup::Fail { error, dir_attr: Some(parent_attr) });
        }

        let mut child_path = parent_path.clone();
        child_path.push(args.name.as_str());
        let child_meta = match Self::metadata(&child_path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(lookup::Fail { error, dir_attr: Some(parent_attr) });
            }
        };

        let child_handle = match self.ensure_handle_for_path(&child_path).await {
            Ok(handle) => handle,
            Err(error) => {
                return Err(lookup::Fail { error, dir_attr: Some(parent_attr) });
            }
        };

        Ok(lookup::Success {
            file: child_handle,
            file_attr: Some(Self::attr_from_metadata(&child_meta)),
            dir_attr: Some(parent_attr),
        })
    }
}
