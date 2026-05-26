use nfs_mamont::vfs::mk_dir;

use super::MockVfs;

impl mk_dir::MkDir for MockVfs {
    async fn mk_dir(
        &self,
        _args: mk_dir::Args,
    ) -> Result<mk_dir::Success, mk_dir::Fail> {
        let handle = self.next_handle();
        Ok(mk_dir::Success {
            file: Some(handle),
            attr: Some(self.config.dir_attr.clone()),
            wcc_data: self.dir_wcc(),
        })
    }
}
