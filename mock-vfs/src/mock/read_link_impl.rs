use nfs_mamont::vfs::file::Path;
use nfs_mamont::vfs::read_link;

use super::MockVfs;

impl read_link::ReadLink for MockVfs {
    async fn read_link(
        &self,
        _args: read_link::Args,
    ) -> Result<read_link::Success, read_link::Fail> {
        Ok(read_link::Success {
            symlink_attr: Some(self.file_attr()),
            data: Path::new("/mock_symlink_target".to_string()).unwrap(),
        })
    }
}
