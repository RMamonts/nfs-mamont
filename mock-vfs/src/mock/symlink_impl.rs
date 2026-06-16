use nfs_mamont::vfs::symlink;

use super::MockVfs;

impl symlink::Symlink for MockVfs {
    async fn symlink(&self, _args: symlink::Args) -> Result<symlink::Success, symlink::Fail> {
        let handle = self.next_handle();
        Ok(symlink::Success {
            file: Some(handle),
            attr: Some(self.file_attr()),
            wcc_data: self.dir_wcc(),
        })
    }
}
