use nfs_mamont::vfs::write;

use super::MockVfs;

impl write::Write for MockVfs {
    async fn write(
        &self,
        _args: write::Args,
    ) -> Result<write::Success, write::Fail> {
        Ok(write::Success {
            file_wcc: self.wcc_data(),
            count: 0,
            committed: write::StableHow::FileSync,
            verifier: write::Verifier([0u8; 8]),
        })
    }
}
