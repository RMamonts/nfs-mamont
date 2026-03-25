use std::os::unix::fs::FileExt;

use nfs_mamont::vfs::read;
use nfs_mamont::Slice;

use super::MirrorFS;

impl read::Read for MirrorFS {
    async fn read(&self, args: read::Args, mut data: Slice) -> Result<read::Success, read::Fail> {
        const SENDFILE_MIN_BYTES: usize = 32 * 1024;

        let (path, attr) = match self.path_and_attr_for_handle(&args.file).await {
            Ok(path_and_attr) => path_and_attr,
            Err(error) => {
                return Err(read::Fail { error, file_attr: None });
            }
        };

        if let Err(error) = Self::validate_regular(&attr) {
            return Err(read::Fail { error, file_attr: Some(attr) });
        }

        let file = match self.get_cached_read_file(&path).await {
            Ok(file) => file,
            Err(error) => {
                return Err(read::Fail { error, file_attr: None });
            }
        };

        let file_len = attr.size;
        let start = args.offset.min(file_len);
        let end = args.offset.saturating_add(args.count as u64).min(file_len);
        let requested = end.saturating_sub(start) as usize;
        let mut read_count = 0usize;
        let mut sendfile_source = None;

        if requested > 0 && sendfile_source.is_none() {
            if let Some(cached) = self.read_ahead_copy_hit(&path, start, requested, &mut data).await {
                read_count = cached;
            }
        }

        #[cfg(target_os = "linux")]
        if requested >= SENDFILE_MIN_BYTES && read_count == 0 {
            read_count = requested;
            sendfile_source = Some(read::SendfileSource { file: file.clone(), offset: start });
            data = Slice::empty();
        }

        if requested > 0 && sendfile_source.is_none() && read_count == 0 {
            let read_file = file.clone();
            let read_result = tokio::task::spawn_blocking(move || {
                let mut remaining = requested;
                let mut local_offset = start;
                let mut local_read_count = 0usize;

                for chunk in data.iter_mut() {
                    if remaining == 0 {
                        break;
                    }

                    let to_read = chunk.len().min(remaining);
                    let mut chunk_offset = 0usize;

                    while chunk_offset < to_read {
                        let bytes = read_file.read_at(
                            &mut chunk[chunk_offset..to_read],
                            local_offset + chunk_offset as u64,
                        )?;

                        if bytes == 0 {
                            return Ok((data, local_read_count));
                        }

                        chunk_offset += bytes;
                        local_read_count += bytes;
                    }

                    local_offset = local_offset.saturating_add(to_read as u64);
                    remaining -= to_read;
                }

                Ok::<(Slice, usize), std::io::Error>((data, local_read_count))
            })
            .await;

            let (filled_data, filled_count) = match read_result {
                Ok(Ok(ok)) => ok,
                Ok(Err(error)) => {
                    return Err(read::Fail {
                        error: Self::io_error_to_vfs(&error),
                        file_attr: Some(attr),
                    });
                }
                Err(_) => {
                    return Err(read::Fail {
                        error: nfs_mamont::vfs::Error::IO,
                        file_attr: Some(attr),
                    });
                }
            };

            data = filled_data;
            read_count = filled_count;
        }

        if read_count > 0 {
            let next_offset = start.saturating_add(read_count as u64);
            let sequential = self.update_read_sequence(&path, start, next_offset).await;
            let should_prefetch = requested >= (SENDFILE_MIN_BYTES * 4) || sequential;
            if should_prefetch {
                self.schedule_read_ahead(&path, file, next_offset, file_len).await;
            }
        }

        Ok(read::Success {
            head: read::SuccessPartial {
                file_attr: Some(attr),
                count: read_count as u32,
                eof: start.saturating_add(read_count as u64) >= file_len,
            },
            data,
            sendfile_source,
        })
    }
}
