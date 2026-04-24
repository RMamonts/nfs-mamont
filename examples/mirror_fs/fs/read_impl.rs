use nfs_mamont::vfs::read;
use nfs_mamont::Slice;

use super::MirrorFS;

impl read::Read for MirrorFS {
    async fn read(&self, args: read::Args, mut data: Slice) -> Result<read::Success, read::Fail> {
        let (_, path, attr) = match self.path_and_attr_for_handle(&args.file).await {
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
        let block_size = super::READ_AHEAD_BLOCK_SIZE as u64;

        let sequential = self.update_read_sequence(&path, start, end).await;
        let small_random =
            requested > 0 && requested <= self.read_path_config.small_io_threshold.get() && !sequential;
        let should_prefetch = !small_random
            && (requested >= self.read_path_config.read_ahead_trigger_bytes.get() || sequential);
        if should_prefetch {
            let start_block_index = end / block_size;
            self.schedule_read_ahead_window(&path, file.clone(), start_block_index, file_len).await;
        }

        if requested > 0 && !small_random {
            if let Some(cached) = self.read_ahead_copy_hit(&path, start, requested, &mut data).await
            {
                read_count = cached;
            }
        }

        if requested > 0 && read_count < requested {
            let read_start = start.saturating_add(read_count as u64);
            let read_remaining = requested - read_count;
            let data_offset = read_count;
            let (filled_data, filled_count) = match self
                .disk_io
                .read_into(file.clone(), read_start, data, data_offset, read_remaining)
                .await
            {
                Ok(ok) => ok,
                Err(error) => {
                    return Err(read::Fail { error, file_attr: Some(attr) });
                }
            };

            data = filled_data;
            read_count += filled_count;
        }

        Ok(read::Success {
            head: read::SuccessPartial {
                file_attr: Some(attr),
                count: read_count as u32,
                eof: start.saturating_add(read_count as u64) >= file_len,
            },
            data,
            sendfile_source: None,
        })
    }
}
