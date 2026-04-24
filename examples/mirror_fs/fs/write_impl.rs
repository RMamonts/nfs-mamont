use async_trait::async_trait;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};

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

        let mut file = match OpenOptions::new().write(true).truncate(false).open(&path) {
            Ok(file) => file,
            Err(error) => {
                return Err(write::Fail {
                    error: Self::io_error_to_vfs(&error),
                    wcc_data: Self::wcc_data(&path, before),
                });
            }
        };

        let data = Self::collect_slice_bytes(&args.data, args.size);
        let count = data.len() as u32;
        if let Err(error) = file.seek(SeekFrom::Start(args.offset)) {
            return Err(write::Fail {
                error: Self::io_error_to_vfs(&error),
                wcc_data: Self::wcc_data(&path, before),
            });
        }
        if let Err(error) = file.write_all(&data) {
            return Err(write::Fail {
                error: Self::io_error_to_vfs(&error),
                wcc_data: Self::wcc_data(&path, before),
            });
        }
        let sync_result = match args.stable {
            write::StableHow::Unstable => Ok(()),
            write::StableHow::DataSync => file.sync_data(),
            write::StableHow::FileSync => file.sync_all(),
        };
        if let Err(error) = sync_result {
            return Err(write::Fail {
                error: Self::io_error_to_vfs(&error),
                wcc_data: Self::wcc_data(&path, before),
            });
        }

        Ok(write::Success {
            file_wcc: Self::wcc_data(&path, before),
            count,
            commited: args.stable,
            verifier: self.write_verifier(),
        })
    }
}
