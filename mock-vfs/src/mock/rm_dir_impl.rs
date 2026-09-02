use nfs_mamont::vfs::rm_dir;

use super::MockVfs;

impl rm_dir::RmDir for MockVfs {
    async fn rm_dir(&self, _args: rm_dir::Args) -> Result<rm_dir::Success, rm_dir::Fail> {
        Ok(rm_dir::Success { wcc_data: self.dir_wcc() })
    }
}
