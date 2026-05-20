use tokio::fs;

use nfs_mamont::vfs::{self, mk_dir};

use super::MirrorFS;

impl mk_dir::MkDir for MirrorFS {
    async fn mk_dir(&self, args: mk_dir::Args) -> Result<mk_dir::Success, mk_dir::Fail> {
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
        let before =
            self.metadata(&dir_path).await.ok().map(|meta| Self::wcc_attr_from_statx(&meta));
        let mut child_path = dir_path.clone();
        child_path.push(args.object.name.as_str());
        if let Err(error) = fs::create_dir(&child_path).await {
            return Err(mk_dir::Fail {
                error: Self::io_error_to_vfs(&error),
                dir_wcc: self.wcc_data(&dir_path, before).await,
            });
        }
        if let Err(error) = Self::apply_set_attr(&child_path, &args.attr) {
            return Err(mk_dir::Fail { error, dir_wcc: self.wcc_data(&dir_path, before).await });
        }
        let attr = match self.metadata(&child_path).await {
            Ok(meta) => Self::attr_from_statx(&meta),
            Err(error) => {
                return Err(mk_dir::Fail { error, dir_wcc: self.wcc_data(&dir_path, before).await })
            }
        };
        let handle = match self.handle_for_path(&child_path).await {
            Ok(handle) => handle,
            Err(error) => {
                return Err(mk_dir::Fail { error, dir_wcc: self.wcc_data(&dir_path, before).await })
            }
        };

        Ok(mk_dir::Success {
            file: Some(handle),
            attr: Some(attr),
            wcc_data: self.wcc_data(&dir_path, before).await,
        })
    }
}
