use nfs_mamont::vfs::read;
use nfs_mamont::Slice;

use super::MockVfs;

impl read::Read for MockVfs {
    async fn read(&self, args: read::Args, mut data: Slice) -> Result<read::Success, read::Fail> {
        let count =
            (args.count as u64).min(self.config.file_size.saturating_sub(args.offset)) as u32;

        let mut offset = 0u64;
        for chunk in data.iter_mut() {
            for byte in chunk.iter_mut() {
                *byte = (offset & 0xFF) as u8;
                offset += 1;
            }
        }

        Ok(read::Success {
            head: read::SuccessPartial {
                file_attr: Some(self.file_attr()),
                count,
                eof: args.offset + count as u64 >= self.config.file_size,
            },
            data,
        })
    }
}
