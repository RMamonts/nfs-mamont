use nfs_mamont::vfs::read;
use nfs_mamont::Buffer;

use super::MockVfs;

impl<B: Buffer> read::Read<B> for MockVfs {
    async fn read(&self, args: read::Args, mut data: B) -> Result<read::Success<B>, read::Fail> {
        if !self.config.latency.is_zero() {
            tokio::time::sleep(self.config.latency).await;
        }

        let count =
            (args.count as u64).min(self.config.file_size.saturating_sub(args.offset)) as u32;

        let mut offset = args.offset;
        for chunk in data.chunks_mut() {
            for byte in chunk.iter_mut() {
                *byte = (offset & 0xFF) as u8;
                offset += 1;
            }
        }

        Ok(read::Success {
            head: read::SuccessHeader {
                file_attr: Some(self.file_attr()),
                count,
                eof: args.offset + count as u64 >= self.config.file_size,
            },
            data,
        })
    }
}
