//! Attribute and file system information procedures.

use nfs_mamont::auth::Credential;
use nfs_mamont::vfs::file::Time;
use nfs_mamont::vfs::{
    self, access, fs_info, fs_stat, get_attr, path_conf, read_link, set_attr, MAX_NAME_LEN,
};

use super::MockVfs;
use crate::tree::ObjectId;

/// Total size of the simulated file system reported by `FSSTAT`: 1 TiB.
const FS_TOTAL_BYTES: u64 = 1 << 40;
/// Number of file slots reported by `FSSTAT`.
const FS_TOTAL_FILES: u64 = 1 << 20;
/// Preferred `READDIR` transfer size reported by `FSINFO`.
const READ_DIR_PREF: u32 = 32 * 1024;
/// Suggested multiple of READ/WRITE sizes reported by `FSINFO`.
const IO_MULT: u32 = 4096;

impl get_attr::GetAttr for MockVfs {
    async fn get_attr(
        &self,
        args: get_attr::Args,
        _cred: &Credential,
    ) -> Result<get_attr::Success, get_attr::Fail> {
        self.wait(0).await;

        Ok(get_attr::Success { object: self.attr_of(&args.file) })
    }
}

impl set_attr::SetAttr for MockVfs {
    /// Accepts and forgets the new attributes; only the `ctime` guard is honoured.
    async fn set_attr(
        &self,
        args: set_attr::Args,
        _cred: &Credential,
    ) -> Result<set_attr::Success, set_attr::Fail> {
        self.wait(0).await;
        let wcc_data = self.wcc(ObjectId::from_handle(&args.file));

        if let Some(guard) = args.guard {
            if !same_time(guard.ctime, self.time) {
                return Err(set_attr::Fail { error: vfs::Error::NotSync, wcc_data });
            }
        }

        Ok(set_attr::Success { wcc_data })
    }
}

impl access::Access for MockVfs {
    /// Grants every requested right to every caller.
    async fn access(
        &self,
        args: access::Args,
        _cred: &Credential,
    ) -> Result<access::Success, access::Fail> {
        self.wait(0).await;

        Ok(access::Success {
            object_attr: Some(self.attr_of(&args.file)),
            access: access::Mask::from_wire(args.mask.bits()),
        })
    }
}

impl read_link::ReadLink for MockVfs {
    /// The tree has no symbolic links, so the object is never one.
    async fn read_link(
        &self,
        args: read_link::Args,
        _cred: &Credential,
    ) -> Result<read_link::Success, read_link::Fail> {
        self.wait(0).await;

        Err(read_link::Fail {
            symlink_attr: Some(self.attr_of(&args.file)),
            error: vfs::Error::InvalidArgument,
        })
    }
}

impl fs_stat::FsStat for MockVfs {
    async fn fs_stat(
        &self,
        args: fs_stat::Args,
        _cred: &Credential,
    ) -> Result<fs_stat::Success, fs_stat::Fail> {
        self.wait(0).await;

        Ok(fs_stat::Success {
            root_attr: Some(self.attr_of(&args.root)),
            total_bytes: FS_TOTAL_BYTES,
            free_bytes: FS_TOTAL_BYTES / 2,
            available_bytes: FS_TOTAL_BYTES / 2,
            total_files: FS_TOTAL_FILES,
            free_files: FS_TOTAL_FILES / 2,
            available_files: FS_TOTAL_FILES / 2,
            invarsec: 0,
        })
    }
}

impl fs_info::FsInfo for MockVfs {
    async fn fs_info(
        &self,
        args: fs_info::Args,
        _cred: &Credential,
    ) -> Result<fs_info::Success, fs_info::Fail> {
        self.wait(0).await;
        let max_io = self.config.max_io;

        Ok(fs_info::Success {
            root_attr: Some(self.attr_of(&args.root)),
            read_max: max_io,
            read_pref: max_io,
            read_mult: IO_MULT,
            write_max: max_io,
            write_pref: max_io,
            write_mult: IO_MULT,
            read_dir_pref: READ_DIR_PREF,
            max_file_size: u64::MAX >> 1,
            time_delta: Time { seconds: 0, nanos: 1 },
            // No hard links and no symbolic links in the tree.
            properties: fs_info::Properties::from_wire(
                fs_info::Properties::HOMOGENEOUS | fs_info::Properties::CANSETTIME,
            ),
        })
    }
}

impl path_conf::PathConf for MockVfs {
    async fn path_conf(
        &self,
        args: path_conf::Args,
        _cred: &Credential,
    ) -> Result<path_conf::Success, path_conf::Fail> {
        self.wait(0).await;

        Ok(path_conf::Success {
            file_attr: Some(self.attr_of(&args.file)),
            link_max: 1,
            name_max: MAX_NAME_LEN as u32,
            no_trunc: true,
            chown_restricted: true,
            case_insensitive: false,
            case_preserving: true,
        })
    }
}

fn same_time(a: Time, b: Time) -> bool {
    a.seconds == b.seconds && a.nanos == b.nanos
}
