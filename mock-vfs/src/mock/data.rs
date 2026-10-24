//! Data procedures: READ, WRITE and COMMIT.

use nfs_mamont::auth::Credential;
use nfs_mamont::vfs::{self, commit, read, write};
use nfs_mamont::Buffer;

use super::MockVfs;
use crate::tree::{Kind, ObjectId};

impl<B: Buffer> read::Read<B> for MockVfs {
    /// Returns `count` bytes (bounded by the buffer and by the file size).
    ///
    /// The buffer content is left untouched unless
    /// [`MockVfsConfig::read_fill`](crate::config::MockVfsConfig::read_fill) is set.
    async fn read(
        &self,
        args: read::Args,
        mut data: B,
        _cred: &Credential,
    ) -> Result<read::Success<B>, read::Fail> {
        let file = ObjectId::from_handle(&args.file);
        let file_attr = Some(self.attr(file));
        if file.kind() != Kind::Regular {
            self.wait(0).await;
            return Err(read::Fail { error: vfs::Error::InvalidArgument, file_attr });
        }

        let file_size = self.config.file_size;
        let available = file_size.saturating_sub(args.offset);
        let count = u64::from(args.count).min(data.len() as u64).min(available) as u32;
        self.wait(u64::from(count)).await;

        if let Some(byte) = self.config.read_fill {
            fill(&mut data, count as usize, byte);
        }
        let eof = args.offset.saturating_add(u64::from(count)) >= file_size;

        Ok(read::Success { head: read::SuccessHeader { file_attr, count, eof }, data })
    }
}

/// Fills the first `count` bytes of `data` with `byte`.
fn fill<B: Buffer>(data: &mut B, count: usize, byte: u8) {
    let mut left = count;
    for chunk in data.chunks_mut() {
        if left == 0 {
            break;
        }
        let take = chunk.len().min(left);
        chunk[..take].fill(byte);
        left -= take;
    }
}

impl<B: Buffer> write::Write<B> for MockVfs {
    /// Accepts the data and drops it; the requested stability level is echoed back,
    /// so clients follow their normal `UNSTABLE` write + `COMMIT` pattern.
    async fn write(
        &self,
        args: write::Args<B>,
        _cred: &Credential,
    ) -> Result<write::Success, write::Fail> {
        let file = ObjectId::from_handle(&args.file);
        let file_wcc = self.wcc(file);
        if file.kind() != Kind::Regular {
            self.wait(0).await;
            return Err(write::Fail { error: vfs::Error::InvalidArgument, wcc_data: file_wcc });
        }

        self.wait(u64::from(args.size)).await;
        // `args.data` is dropped here, which returns its buffers to the allocator.

        Ok(write::Success {
            file_wcc,
            count: args.size,
            committed: args.stable,
            verifier: self.write_verifier(),
        })
    }
}

impl commit::Commit for MockVfs {
    async fn commit(
        &self,
        args: commit::Args,
        _cred: &Credential,
    ) -> Result<commit::Success, commit::Fail> {
        self.wait(0).await;

        Ok(commit::Success {
            file_wcc: self.wcc(ObjectId::from_handle(&args.file)),
            verifier: self.write_verifier(),
        })
    }
}
