use nfs_mamont::vfs::link;

use super::MockVfs;

impl link::Link for MockVfs {
    async fn link(&self, _args: link::Args) -> Result<link::Success, link::Fail> {
        Ok(link::Success { file_attr: Some(self.file_attr()), dir_wcc: self.dir_wcc() })
    }
}
