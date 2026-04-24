use nfs_mamont::consts::nfsv3::NFS3_WRITEVERFSIZE;
use nfs_mamont::vfs::{self, read_dir, read_dir_plus};

use super::MirrorFS;

impl read_dir_plus::ReadDirPlus for MirrorFS {
    async fn read_dir_plus(
        &self,
        args: read_dir_plus::Args,
    ) -> Result<read_dir_plus::Success, read_dir_plus::Fail> {
        let _ = args;
        Err(read_dir_plus::Fail { error: vfs::Error::NotSupported, dir_attr: None })
    }
}
