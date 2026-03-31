use async_trait::async_trait;
use std::path::Path;
use tokio::fs::OpenOptions;

use super::{MirrorFS, DEFAULT_SET_ATTR};
use nfs_mamont::vfs::create::{Fail, How, Success};
use nfs_mamont::vfs::{self, create};

#[async_trait]
impl create::Create for MirrorFS {
    async fn create(&self, path: &Path, mode: How) -> Result<Success, Fail> {
        if !path.is_file() {
            return Err(create::Fail {
                error: vfs::Error::BadType,
                wcc_data: vfs::WccData { before: None, after: None },
            });
        }
        let dir_path = match path.parent() {
            Some(parent) if parent.is_dir() => parent,
            _ => {
                return Err(create::Fail {
                    error: vfs::Error::BadType,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };
        let dir_meta = match Self::metadata(dir_path) {
            Ok(meta) => meta,
            Err(error) => {
                return Err(create::Fail {
                    error,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = Some(Self::wcc_attr_from_metadata(&dir_meta));
        let dir_attr = Self::attr_from_metadata(&dir_meta);
        if let Err(error) = Self::validate_directory(&dir_attr) {
            return Err(create::Fail { error, wcc_data: Self::wcc_data(dir_path, before) });
        }

        let existed = std::fs::symlink_metadata(path).is_ok();

        let apply_attr = match mode {
            create::How::Unchecked(attr) => {
                if !existed {
                    if let Err(error) =
                        OpenOptions::new().write(true).create(true).truncate(false).open(path).await
                    {
                        return Err(create::Fail {
                            error: Self::io_error_to_vfs(&error),
                            wcc_data: Self::wcc_data(dir_path, before),
                        });
                    }
                }
                attr
            }
            create::How::Guarded(attr) => {
                if existed {
                    return Err(create::Fail {
                        error: vfs::Error::Exist,
                        wcc_data: Self::wcc_data(dir_path, before),
                    });
                }
                if let Err(error) = OpenOptions::new().write(true).create_new(true).open(path).await
                {
                    return Err(create::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: Self::wcc_data(dir_path, before),
                    });
                }
                attr
            }
            create::How::Exclusive(ref verifier) => {
                match OpenOptions::new().write(true).create_new(true).open(path).await {
                    Ok(_) => {
                        Self::store_exclusive_verifier(path, &verifier.0);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        if !Self::check_exclusive_verifier(path, &verifier.0) {
                            return Err(create::Fail {
                                error: vfs::Error::Exist,
                                wcc_data: Self::wcc_data(dir_path, before),
                            });
                        }
                    }
                    Err(error) => {
                        return Err(create::Fail {
                            error: Self::io_error_to_vfs(&error),
                            wcc_data: Self::wcc_data(dir_path, before),
                        });
                    }
                }
                DEFAULT_SET_ATTR
            }
        };

        if let Err(error) = Self::apply_set_attr(path, &apply_attr) {
            return Err(create::Fail { error, wcc_data: Self::wcc_data(dir_path, before) });
        }

        let attr = match Self::metadata(path) {
            Ok(meta) => Self::attr_from_metadata(&meta),
            Err(error) => {
                return Err(create::Fail { error, wcc_data: Self::wcc_data(dir_path, before) });
            }
        };
        let handle = match self.ensure_handle_for_path(path).await {
            Ok(handle) => handle,
            Err(error) => {
                return Err(create::Fail { error, wcc_data: Self::wcc_data(dir_path, before) });
            }
        };

        Ok(create::Success {
            file: Some(handle),
            attr: Some(attr),
            wcc_data: Self::wcc_data(dir_path, before),
        })
    }
}
