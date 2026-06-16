use nfs_mamont::vfs::lookup;

use super::MockVfs;

impl lookup::Lookup for MockVfs {
    async fn lookup(&self, _args: lookup::Args) -> Result<lookup::Success, lookup::Fail> {
        let handle = self.next_handle();
        Ok(lookup::Success {
            file: handle,
            file_attr: Some(self.file_attr()),
            dir_attr: Some(self.config.dir_attr.clone()),
        })
    }
}
