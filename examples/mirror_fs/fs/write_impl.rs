use std::os::unix::fs::FileExt;

use nfs_mamont::vfs::{self, write};

use super::MirrorFS;

impl write::Write for MirrorFS {
    async fn write(&self, args: write::Args) -> Result<write::Success, write::Fail> {
        let (_, path, attr) = match self.path_and_attr_for_handle(&args.file).await {
            Ok(path_and_attr) => path_and_attr,
            Err(error) => {
                return Err(write::Fail {
                    error,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };
        if let Err(error) = Self::validate_regular(&attr) {
            return Err(write::Fail {
                error,
                wcc_data: vfs::WccData {
                    before: Some(Self::wcc_attr_from_attr(&attr)),
                    after: Some(attr),
                },
            });
        }

        let before = Some(Self::wcc_attr_from_attr(&attr));
        let file = match self.get_cached_write_file(&path).await {
            Ok(file) => file,
            Err(error) => {
                return Err(write::Fail { error, wcc_data: vfs::WccData { before, after: None } });
            }
        };

        let stable = args.stable;
        let size = args.size as usize;
        let offset = args.offset;
        let data = args.data;
        let write_result = tokio::task::spawn_blocking(move || -> Result<_, std::io::Error> {
            let mut remaining_payload = size;
            let mut local_offset = offset;
            let mut written = 0usize;

            for part in &data {
                if remaining_payload == 0 {
                    break;
                }

                let to_write = part.len().min(remaining_payload);
                let mut chunk_offset = 0usize;
                while chunk_offset < to_write {
                    let bytes = file.write_at(
                        &part[chunk_offset..to_write],
                        local_offset + chunk_offset as u64,
                    )?;
                    if bytes == 0 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::WriteZero,
                            "write_at returned zero bytes",
                        ));
                    }
                    chunk_offset += bytes;
                    written += bytes;
                }

                local_offset = local_offset.saturating_add(to_write as u64);
                remaining_payload -= to_write;
            }

            match stable {
                write::StableHow::Unstable => {}
                write::StableHow::DataSync => {
                    file.sync_data()?;
                }
                write::StableHow::FileSync => {
                    file.sync_all()?;
                }
            }

            Ok((file.metadata()?, written as u32))
        })
        .await;

        let (after_meta, written) = match write_result {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                return Err(write::Fail {
                    error: Self::io_error_to_vfs(&error),
                    wcc_data: vfs::WccData { before, after: Self::file_attr(&path) },
                });
            }
            Err(_) => {
                return Err(write::Fail {
                    error: vfs::Error::IO,
                    wcc_data: vfs::WccData { before, after: Self::file_attr(&path) },
                });
            }
        };
        let after_attr = Self::attr_from_metadata(&after_meta);
        self.store_attr_cache_metadata(path.clone(), after_meta).await;

        match stable {
            write::StableHow::Unstable => {
                self.mark_pending_unstable_write(&path).await;
            }
            write::StableHow::DataSync => {
                self.clear_pending_unstable_write(&path).await;
            }
            write::StableHow::FileSync => {
                self.clear_pending_unstable_write(&path).await;
            }
        }

        self.invalidate_read_ahead_path(&path).await;

        Ok(write::Success {
            file_wcc: vfs::WccData { before, after: Some(after_attr) },
            count: written,
            commited: stable,
            verifier: self.write_verifier(),
        })
    }
}
