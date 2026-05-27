use nfs_mamont::vfs::remove;

use super::MockVfs;

impl remove::Remove for MockVfs {
    async fn remove(&self, _args: remove::Args) -> Result<remove::Success, remove::Fail> {
        Ok(remove::Success { wcc_data: self.dir_wcc() })
    }
}
