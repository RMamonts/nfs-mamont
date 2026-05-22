use nfs_mamont::vfs::{self, create};

use super::{MirrorFS, DEFAULT_SET_ATTR};

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
        let dir_meta = match self.metadata(&dir_path).await {
            Ok(meta) => meta,
            Err(error) => {
                return Err(create::Fail {
                    error,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };
        let before = Some(Self::wcc_attr_from_statx(&dir_meta));
        let dir_attr = Self::attr_from_statx(&dir_meta);
        if let Err(error) = Self::validate_directory(&dir_attr) {
            return Err(create::Fail { error, wcc_data: self.wcc_data(&dir_path, before).await });
        }

        let mut child_path = dir_path.clone();
        child_path.push(args.object.name.as_str());
        let existed = match self.metadata(&child_path).await {
            Ok(_) => true,
            Err(vfs::Error::NoEntry) => false,
            Err(error) => {
                return Err(create::Fail {
                    error,
                    wcc_data: self.wcc_data(&dir_path, before).await,
                })
            }
        };

        let apply_attr = match &args.how {
            create::How::Unchecked(attr) => {
                if !existed {
                    if let Err(error) = std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(&child_path)
                    {
                        return Err(create::Fail {
                            error: Self::io_error_to_vfs(&error),
                            wcc_data: self.wcc_data(&dir_path, before).await,
                        });
                    }
                }
                attr
            }
            create::How::Guarded(attr) => {
                if existed {
                    return Err(create::Fail {
                        error: vfs::Error::Exist,
                        wcc_data: self.wcc_data(&dir_path, before).await,
                    });
                }
                if let Err(error) =
                    std::fs::OpenOptions::new().write(true).create_new(true).open(&child_path)
                {
                    return Err(create::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: self.wcc_data(&dir_path, before).await,
                    });
                }
                attr
            }
            create::How::Exclusive(ref verifier) => {
                match std::fs::OpenOptions::new().write(true).create_new(true).open(&child_path) {
                    Ok(_) => {
                        Self::store_exclusive_verifier(&child_path, &verifier.0);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        if !Self::check_exclusive_verifier(&child_path, &verifier.0) {
                            return Err(create::Fail {
                                error: vfs::Error::Exist,
                                wcc_data: self.wcc_data(&dir_path, before).await,
                            });
                        }
                    }
                    Err(error) => {
                        return Err(create::Fail {
                            error: Self::io_error_to_vfs(&error),
                            wcc_data: self.wcc_data(&dir_path, before).await,
                        });
                    }
                }
                &DEFAULT_SET_ATTR
            }
        };

        if let Err(error) = Self::apply_set_attr(&child_path, apply_attr) {
            return Err(create::Fail { error, wcc_data: self.wcc_data(&dir_path, before).await });
        }

        let attr = match self.metadata(&child_path).await {
            Ok(meta) => Self::attr_from_statx(&meta),
            Err(error) => {
                return Err(create::Fail {
                    error,
                    wcc_data: self.wcc_data(&dir_path, before).await,
                });
            }
        };
        let handle = match self.handle_for_path(&child_path).await {
            Ok(handle) => handle,
            Err(error) => {
                return Err(create::Fail {
                    error,
                    wcc_data: self.wcc_data(&dir_path, before).await,
                });
            }
        };

        Ok(create::Success {
            file: Some(handle),
            attr: Some(attr),
            wcc_data: self.wcc_data(&dir_path, before).await,
        })
    }
}
