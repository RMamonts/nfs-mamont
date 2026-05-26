use nfs_mamont::vfs::file::Name;
use nfs_mamont::vfs::read_dir;
use nfs_mamont::vfs::read_dir_plus;

use super::MockVfs;

impl read_dir_plus::ReadDirPlus for MockVfs {
    async fn read_dir_plus(
        &self,
        _args: read_dir_plus::Args,
    ) -> Result<read_dir_plus::Success, read_dir_plus::Fail> {
        let count = self.config.dir_entry_count;
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let name = Name::new(format!("entry_{}", i)).unwrap();
            let handle = self.next_handle();
            entries.push(read_dir_plus::Entry {
                file_id: (i + 1) as u64,
                file_name: name,
                cookie: read_dir::Cookie::new((i + 1) as u64),
                file_attr: Some(self.file_attr()),
                file_handle: Some(handle),
            });
        }

        Ok(read_dir_plus::Success {
            dir_attr: Some(self.config.dir_attr.clone()),
            cookie_verifier: read_dir::CookieVerifier::new([0u8; 8]),
            entries,
            eof: true,
        })
    }
}
