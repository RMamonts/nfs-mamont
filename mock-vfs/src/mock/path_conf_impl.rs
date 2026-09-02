use nfs_mamont::vfs::path_conf;

use super::MockVfs;

impl path_conf::PathConf for MockVfs {
    async fn path_conf(
        &self,
        _args: path_conf::Args,
    ) -> Result<path_conf::Success, path_conf::Fail> {
        Ok(path_conf::Success {
            file_attr: Some(self.file_attr()),
            link_max: u32::MAX,
            name_max: 255,
            no_trunc: true,
            chown_restricted: true,
            case_insensitive: false,
            case_preserving: true,
        })
    }
}
