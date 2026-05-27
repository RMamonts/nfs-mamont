use nfs_mamont::vfs::commit;
use nfs_mamont::vfs::write;

use super::MockVfs;

impl commit::Commit for MockVfs {
    async fn commit(&self, _args: commit::Args) -> Result<commit::Success, commit::Fail> {
        Ok(commit::Success { file_wcc: self.wcc_data(), verifier: write::Verifier([0u8; 8]) })
    }
}
