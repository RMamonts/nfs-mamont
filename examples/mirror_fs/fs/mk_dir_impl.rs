use async_trait::async_trait;
use std::path::Path;
use tokio::fs;

use super::MirrorFS;
use nfs_mamont::vfs::set_attr::NewAttr;
use nfs_mamont::vfs::{self, mk_dir};

#[async_trait]
impl mk_dir::MkDir for MirrorFS {
    async fn mk_dir(&self, path: &Path, attr: NewAttr) -> Result<mk_dir::Success, mk_dir::Fail> {
        //TODO(make ensure path?)
        let dir_path = match path.parent() {
            Some(parent) if parent.is_dir() => parent,
            _ => {
                return Err(mk_dir::Fail {
                    error: vfs::Error::BadType,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };

        let before = std::fs::symlink_metadata(&dir_path)
            .ok()
            .map(|meta| Self::wcc_attr_from_metadata(&meta));

        if let Err(error) = fs::create_dir(path).await {
            return Err(mk_dir::Fail {
                error: Self::io_error_to_vfs(&error),
                dir_wcc: Self::wcc_data(&dir_path, before),
            });
        }
        if let Err(error) = Self::apply_set_attr(path, &attr) {
            return Err(mk_dir::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) });
        }
        let attr = match Self::metadata(path) {
            Ok(meta) => Self::attr_from_metadata(&meta),
            Err(error) => {
                return Err(mk_dir::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) })
            }
        };
        let handle = match self.ensure_handle_for_path(path).await {
            Ok(handle) => handle,
            Err(error) => {
                return Err(mk_dir::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) })
            }
        };

        Ok(mk_dir::Success {
            file: Some(handle),
            attr: Some(attr),
            wcc_data: Self::wcc_data(&dir_path, before),
        })
    }
}
