use nfs_mamont::vfs::commit;

use super::*;

impl commit::Commit for MirrorFS {
    async fn commit(&self, args: commit::Args) -> Result<commit::Success, commit::Fail> {
        let _ = args;
        Ok(commit::Success {
            file_wcc: vfs::WccData { before: None, after: None },
            verifier: self.write_verifier(),
        })
    }
}
