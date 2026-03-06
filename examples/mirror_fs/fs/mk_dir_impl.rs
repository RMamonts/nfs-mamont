use async_trait::async_trait;
use tokio::fs;

use nfs_mamont::vfs::mk_dir;

use super::*;

#[async_trait]
impl mk_dir::MkDir for MirrorFS {
    async fn mk_dir(&self, args: mk_dir::Args) -> mk_dir::Result {
        if let Err(error) = Self::ensure_name_allowed(&args.object.name) {
            return Err(mk_dir::Fail {
                error,
                dir_wcc: vfs::WccData { before: None, after: None },
            });
        }
        let dir_path = match self.path_for_handle(&args.object.dir).await {
            Ok(path) => path,
            Err(error) => {
                return Err(mk_dir::Fail {
                    error,
                    dir_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = std::fs::symlink_metadata(&dir_path).ok().map(|meta| Self::wcc_attr_from_metadata(&meta));
        let mut child_path = dir_path.clone();
        child_path.push(args.object.name.as_str());
        if let Err(error) = fs::create_dir(&child_path).await {
            return Err(mk_dir::Fail {
                error: Self::io_error_to_vfs(&error),
                dir_wcc: Self::wcc_data(&dir_path, before),
            });
        }
        if let Err(error) = Self::apply_set_attr(&child_path, &args.attr) {
            return Err(mk_dir::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) });
        }
        let attr = match Self::metadata(&child_path) {
            Ok(meta) => Self::attr_from_metadata(&meta),
            Err(error) => return Err(mk_dir::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) }),
        };
        let handle = match self.ensure_handle_for_path(&child_path).await {
            Ok(handle) => handle,
            Err(error) => return Err(mk_dir::Fail { error, dir_wcc: Self::wcc_data(&dir_path, before) }),
        };

        Ok(mk_dir::Success {
            file: Some(handle),
            attr: Some(attr),
            wcc_data: Self::wcc_data(&dir_path, before),
        })
    }
}
