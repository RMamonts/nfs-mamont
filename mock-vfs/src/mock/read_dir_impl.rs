use nfs_mamont::vfs::file::Name;
use nfs_mamont::vfs::read_dir;

use super::MockVfs;

impl read_dir::ReadDir for MockVfs {
    async fn read_dir(&self, _args: read_dir::Args) -> Result<read_dir::Success, read_dir::Fail> {
        let count = self.config.dir_entry_count;
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let name = Name::new(format!("entry_{}", i)).unwrap();
            entries.push(read_dir::Entry {
                file_id: (i + 1) as u64,
                file_name: name,
                cookie: read_dir::Cookie::new((i + 1) as u64),
            });
        }

        Ok(read_dir::Success {
            dir_attr: Some(self.config.dir_attr.clone()),
            cookie_verifier: read_dir::CookieVerifier::new([0u8; 8]),
            entries,
            eof: true,
        })
    }
}
