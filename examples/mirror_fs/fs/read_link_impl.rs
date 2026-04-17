use async_trait::async_trait;

use nfs_mamont::vfs::{self, file, read_link};

use super::MirrorFS;

#[async_trait]
impl read_link::ReadLink for MirrorFS {
    async fn read_link(
        &self,
        args: read_link::Args,
    ) -> Result<read_link::Success, read_link::Fail> {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(read_link::Fail { error, symlink_attr: None });
            }
        };
        let meta = match Self::metadata(&path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(read_link::Fail { error, symlink_attr: None });
            }
        };
        let attr = Self::attr_from_metadata(&meta);
        if !matches!(attr.file_type, file::Type::Symlink) {
            return Err(read_link::Fail {
                error: vfs::Error::InvalidArgument,
                symlink_attr: Some(attr),
            });
        }
        let target = match std::fs::read_link(&path) {
            Ok(target) => target,
            Err(error) => {
                return Err(read_link::Fail {
                    error: Self::io_error_to_vfs(&error),
                    symlink_attr: Some(attr),
                });
            }
        };
        let target_str = match target.to_str() {
            Some(s) => s.to_owned(),
            None => {
                return Err(read_link::Fail { error: vfs::Error::IO, symlink_attr: Some(attr) });
            }
        };
        let data = match file::Path::new(target_str) {
            Ok(path) => path,
            Err(_) => {
                return Err(read_link::Fail {
                    error: vfs::Error::InvalidArgument,
                    symlink_attr: Some(attr),
                });
            }
        };

        Ok(read_link::Success { symlink_attr: Some(attr), data })
    }
}
