use nfs_mamont::vfs::{self, path_conf};

use super::MirrorFS;

impl path_conf::PathConf for MirrorFS {
    async fn path_conf(
        &self,
        args: path_conf::Args,
    ) -> Result<path_conf::Success, path_conf::Fail> {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => return Err(path_conf::Fail { error, file_attr: None }),
        };
        Ok(path_conf::Success {
            file_attr: Self::file_attr(&path),
            link_max: u32::MAX,
            name_max: vfs::MAX_NAME_LEN as u32,
            no_trunc: true,
            chown_restricted: true,
            case_insensitive: false,
            case_preserving: true,
        })
    }
}
