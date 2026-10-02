use nfs_mamont::auth::Credential;
use nfs_mamont::vfs::access;

use super::MockVfs;

impl access::Access for MockVfs {
    async fn access(
        &self,
        _args: access::Args,
        _cred: &Credential,
    ) -> Result<access::Success, access::Fail> {
        Ok(access::Success {
            object_attr: Some(self.file_attr()),
            access: access::Mask::from_wire(access::Mask::ALL),
        })
    }
}
