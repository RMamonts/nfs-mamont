use async_trait::async_trait;
use tokio::fs::OpenOptions;

use nfs_mamont::vfs::{self, create};

use super::{MirrorFS, DEFAULT_SET_ATTR};

#[async_trait]
impl create::Create for MirrorFS {
    async fn create(&self, args: create::Args) -> Result<create::Success, create::Fail> {
        if let Err(error) = Self::ensure_name_allowed(&args.object.name) {
            return Err(create::Fail {
                error,
                wcc_data: vfs::WccData { before: None, after: None },
            });
        }

        let dir_path = match self.path_for_handle(&args.object.dir).await {
            Ok(path) => path,
            Err(error) => {
                return Err(create::Fail {
                    error,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = std::fs::symlink_metadata(&dir_path)
            .ok()
            .map(|meta| Self::wcc_attr_from_metadata(&meta));
        let dir_meta = match Self::metadata(&dir_path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(create::Fail { error, wcc_data: Self::wcc_data(&dir_path, before) });
            }
        };
        let dir_attr = Self::attr_from_metadata(&dir_meta);
        if let Err(error) = Self::validate_directory(&dir_attr) {
            return Err(create::Fail { error, wcc_data: Self::wcc_data(&dir_path, before) });
        }

        let mut child_path = dir_path.clone();
        child_path.push(args.object.name.as_str());
        let existed = std::fs::symlink_metadata(&child_path).is_ok();

        let apply_attr = match &args.how {
            create::How::Unchecked(attr) => {
                if !existed {
                    if let Err(error) = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(&child_path)
                        .await
                    {
                        return Err(create::Fail {
                            error: Self::io_error_to_vfs(&error),
                            wcc_data: Self::wcc_data(&dir_path, before),
                        });
                    }
                }
                attr
            }
            create::How::Guarded(attr) => {
                if existed {
                    return Err(create::Fail {
                        error: vfs::Error::Exist,
                        wcc_data: Self::wcc_data(&dir_path, before),
                    });
                }
                if let Err(error) =
                    OpenOptions::new().write(true).create_new(true).open(&child_path).await
                {
                    return Err(create::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: Self::wcc_data(&dir_path, before),
                    });
                }
                attr
            }
            create::How::Exclusive(ref verifier) => {
                match OpenOptions::new().write(true).create_new(true).open(&child_path).await {
                    Ok(_) => {
                        Self::store_exclusive_verifier(&child_path, &verifier.0);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        if !Self::check_exclusive_verifier(&child_path, &verifier.0) {
                            return Err(create::Fail {
                                error: vfs::Error::Exist,
                                wcc_data: Self::wcc_data(&dir_path, before),
                            });
                        }
                    }
                    Err(error) => {
                        return Err(create::Fail {
                            error: Self::io_error_to_vfs(&error),
                            wcc_data: Self::wcc_data(&dir_path, before),
                        });
                    }
                }
                &DEFAULT_SET_ATTR
            }
        };

        if let Err(error) = Self::apply_set_attr(&child_path, apply_attr) {
            return Err(create::Fail { error, wcc_data: Self::wcc_data(&dir_path, before) });
        }

        let attr = match Self::metadata(&child_path) {
            Ok(meta) => Self::attr_from_metadata(&meta),
            Err(error) => {
                return Err(create::Fail { error, wcc_data: Self::wcc_data(&dir_path, before) });
            }
        };
        let handle = match self.ensure_handle_for_path(&child_path).await {
            Ok(handle) => handle,
            Err(error) => {
                return Err(create::Fail { error, wcc_data: Self::wcc_data(&dir_path, before) });
            }
        };

        Ok(create::Success {
            file: Some(handle),
            attr: Some(attr),
            wcc_data: Self::wcc_data(&dir_path, before),
        })
    }
}
