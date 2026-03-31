use std::path::Path;

use async_trait::async_trait;
use nfs_mamont::vfs;
use nfs_mamont::vfs::file::Handle;
use nfs_mamont::vfs::lookup;

use super::MirrorFS;

#[async_trait]
impl lookup::Lookup for MirrorFS {
    async fn lookup(&self, path: &Path) -> Result<lookup::Success, lookup::Fail> {
        let parent_path = match path.parent() {
            Some(parent) if parent.is_dir() => parent,
            _ => {
                return Err(lookup::Fail { error: vfs::Error::BadType, dir_attr: None });
            }
        };
        let parent_meta = match Self::metadata(parent_path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(lookup::Fail { error, dir_attr: None });
            }
        };
        let parent_attr = Self::attr_from_metadata(&parent_meta);
        if let Err(error) = Self::validate_directory(&parent_attr) {
            return Err(lookup::Fail { error, dir_attr: Some(parent_attr) });
        }

        //TODO(make ensure path?)

        let child_meta = match Self::metadata(path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(lookup::Fail { error, dir_attr: Some(parent_attr) });
            }
        };

        Ok(lookup::Success {
            //TODO("change!!!")
            file: Handle(5u64.to_be_bytes()),
            file_attr: Some(Self::attr_from_metadata(&child_meta)),
            dir_attr: Some(parent_attr),
        })
    }
}
