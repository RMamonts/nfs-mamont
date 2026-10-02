use nfs_mamont::auth::Credential;
use nfs_mamont::vfs::write;
use nfs_mamont::Buffer;

use super::MockVfs;

impl<B: Buffer> write::Write<B> for MockVfs {
    async fn write(
        &self,
        args: write::Args<B>,
        _cred: &Credential,
    ) -> Result<write::Success, write::Fail> {
        if !self.config.latency.is_zero() {
            tokio::time::sleep(self.config.latency).await;
        }

        Ok(write::Success {
            file_wcc: self.wcc_data(),
            count: args.size,
            committed: write::StableHow::FileSync,
            verifier: write::Verifier([0u8; 8]),
        })
    }
}
