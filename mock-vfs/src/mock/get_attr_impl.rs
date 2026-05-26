use nfs_mamont::vfs::get_attr;

use super::MockVfs;

impl get_attr::GetAttr for MockVfs {
    async fn get_attr(
        &self,
        _args: get_attr::Args,
    ) -> Result<get_attr::Success, get_attr::Fail> {
        Ok(get_attr::Success { object: self.file_attr() })
    }
}
