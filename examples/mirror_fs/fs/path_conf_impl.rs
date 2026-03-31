use async_trait::async_trait;
use std::path::Path;

use nfs_mamont::vfs::{self, path_conf};

use super::MirrorFS;

#[async_trait]
impl path_conf::PathConf for MirrorFS {
    async fn path_conf(&self, path: &Path) -> Result<path_conf::Success, path_conf::Fail> {
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
