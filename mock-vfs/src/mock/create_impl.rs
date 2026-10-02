use nfs_mamont::auth::Credential;
use nfs_mamont::vfs::create;

use super::MockVfs;

impl create::Create for MockVfs {
    async fn create(
        &self,
        _args: create::Args,
        _cred: &Credential,
    ) -> Result<create::Success, create::Fail> {
        let handle = self.next_handle();
        Ok(create::Success {
            file: Some(handle),
            attr: Some(self.file_attr()),
            wcc_data: self.dir_wcc(),
        })
    }
}
