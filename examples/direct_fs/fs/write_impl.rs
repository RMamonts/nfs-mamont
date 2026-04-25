use async_trait::async_trait;

use nfs_mamont::vfs::{self, write};

use super::MirrorFS;

#[async_trait]
impl write::Write for MirrorFS {
    async fn write(&self, args: write::Args) -> Result<write::Success, write::Fail> {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(write::Fail {
                    error,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };

        let before_meta = std::fs::symlink_metadata(&path).ok();
        let before = before_meta.as_ref().map(Self::wcc_attr_from_metadata);
        if let Some(attr) = before_meta.as_ref().map(Self::attr_from_metadata) {
            if let Err(error) = Self::validate_regular(&attr) {
                return Err(write::Fail { error, wcc_data: Self::wcc_data(&path, before) });
            }
        }

        let data = Self::collect_slice_bytes(&args.data, args.size);
        let try_direct = self.should_try_direct_io(args.offset, data.len());

        let file = match Self::open_for_write(&path, try_direct) {
            Ok(file) => file,
            Err(error) => {
                return Err(write::Fail {
                    error: Self::io_error_to_vfs(&error),
                    wcc_data: Self::wcc_data(&path, before),
                });
            }
        };

        let write_result = Self::write_all_at_blocking(&file, args.offset, &data);
        let file = if write_result.is_ok() {
            file
        } else if try_direct {
            match Self::open_for_write(&path, false) {
                Ok(file) => {
                    if let Err(error) = Self::write_all_at_blocking(&file, args.offset, &data) {
                        return Err(write::Fail {
                            error: Self::io_error_to_vfs(&error),
                            wcc_data: Self::wcc_data(&path, before),
                        });
                    }
                    file
                }
                Err(error) => {
                    return Err(write::Fail {
                        error: Self::io_error_to_vfs(&error),
                        wcc_data: Self::wcc_data(&path, before),
                    });
                }
            }
        } else {
            let error = write_result.err().unwrap();
            return Err(write::Fail {
                error: Self::io_error_to_vfs(&error),
                wcc_data: Self::wcc_data(&path, before),
            });
        };

        if let Err(error) = Self::sync_file(&file, args.stable) {
            return Err(write::Fail {
                error: Self::io_error_to_vfs(&error),
                wcc_data: Self::wcc_data(&path, before),
            });
        }

        Ok(write::Success {
            file_wcc: Self::wcc_data(&path, before),
            count: data.len() as u32,
            commited: args.stable,
            verifier: self.write_verifier(),
        })
    }
}
