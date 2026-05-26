use nfs_mamont::vfs::rename;

use super::MockVfs;

impl rename::Rename for MockVfs {
    async fn rename(
        &self,
        _args: rename::Args,
    ) -> Result<rename::Success, rename::Fail> {
        Ok(rename::Success {
            from_dir_wcc: self.dir_wcc(),
            to_dir_wcc: self.dir_wcc(),
        })
    }
}
