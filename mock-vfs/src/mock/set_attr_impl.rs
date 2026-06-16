use nfs_mamont::vfs::set_attr;

use super::MockVfs;

impl set_attr::SetAttr for MockVfs {
    async fn set_attr(&self, _args: set_attr::Args) -> Result<set_attr::Success, set_attr::Fail> {
        Ok(set_attr::Success { wcc_data: self.wcc_data() })
    }
}
