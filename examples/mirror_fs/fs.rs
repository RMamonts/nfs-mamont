use nfs_mamont::consts::nfsv3::{NFS3_COOKIEVERFSIZE, NFS3_CREATEVERFSIZE};
use nfs_mamont::vfs;
use nfs_mamont::vfs::file;
use nfs_mamont::vfs::read_dir;
use nfs_mamont::vfs::set_attr;
use nfs_mamont::vfs::write;
use nfs_mamont::Slice;
use std::fs::Metadata;
use std::io::ErrorKind;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod access_impl;
mod commit_impl;
mod create_impl;
mod fs_info_impl;
mod fs_stat_impl;
mod get_attr_impl;
mod link_impl;
mod lookup_impl;
mod mk_dir_impl;
mod mk_node_impl;
mod path_conf_impl;
mod read_dir_impl;
mod read_dir_plus_impl;
mod read_impl;
mod read_link_impl;
mod remove_impl;
mod rename_impl;
mod rm_dir_impl;
mod set_attr_impl;
mod symlink_impl;
mod write_impl;

const READ_WRITE_MAX: u32 = 64 * 1024;
const READ_DIR_PREF: u32 = 8 * 1024;
const DEFAULT_SET_ATTR: set_attr::NewAttr = set_attr::NewAttr {
    mode: None,
    uid: None,
    gid: None,
    size: None,
    atime: set_attr::SetTime::DontChange,
    mtime: set_attr::SetTime::DontChange,
};

/// A file system implementation that mirrors a local directory.
#[derive(Debug)]
pub struct MirrorFS {
    generation: u64,
}

impl MirrorFS {
    /// Creates a new mirror file system with the given root path.
    pub fn new() -> Self {
        let generation =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_nanos()
                as u64;
        Self { generation }
    }

    /// Returns the root handle.
    pub async fn root_handle(&self) -> file::Handle {
        file::Handle(1u64.to_be_bytes())
    }

    fn write_verifier(&self) -> write::Verifier {
        write::Verifier(self.generation.to_be_bytes())
    }

    fn cookie_verifier_for_attr(attr: &file::Attr) -> read_dir::CookieVerifier {
        let mut raw = [0u8; NFS3_COOKIEVERFSIZE];
        raw[..4].copy_from_slice(&attr.ctime.seconds.to_be_bytes());
        raw[4..].copy_from_slice(&attr.ctime.nanos.to_be_bytes());
        read_dir::CookieVerifier::new(raw)
    }

    fn io_error_to_vfs(error: &std::io::Error) -> vfs::Error {
        match error.kind() {
            ErrorKind::NotFound => vfs::Error::NoEntry,
            ErrorKind::PermissionDenied => vfs::Error::Access,
            ErrorKind::AlreadyExists => vfs::Error::Exist,
            ErrorKind::InvalidInput | ErrorKind::InvalidData => vfs::Error::InvalidArgument,
            ErrorKind::DirectoryNotEmpty => vfs::Error::NotEmpty,
            ErrorKind::IsADirectory => vfs::Error::IsDir,
            ErrorKind::NotADirectory => vfs::Error::NotDir,
            ErrorKind::WriteZero => vfs::Error::NoSpace,
            _ => vfs::Error::IO,
        }
    }

    fn time_from_unix(seconds: i64, nanos: i64) -> file::Time {
        file::Time {
            seconds: seconds.max(0).min(u32::MAX as i64) as u32,
            nanos: nanos.max(0).min(u32::MAX as i64) as u32,
        }
    }

    fn system_time_from_file_time(time: file::Time) -> SystemTime {
        UNIX_EPOCH + Duration::new(u64::from(time.seconds), time.nanos)
    }

    fn same_time(left: file::Time, right: file::Time) -> bool {
        left.seconds == right.seconds && left.nanos == right.nanos
    }

    fn attr_from_metadata(meta: &Metadata) -> file::Attr {
        let file_type = meta.file_type();
        let file_type = if file_type.is_dir() {
            file::Type::Directory
        } else if file_type.is_symlink() {
            file::Type::Symlink
        } else if file_type.is_file() {
            file::Type::Regular
        } else if file_type.is_block_device() {
            file::Type::BlockDevice
        } else if file_type.is_char_device() {
            file::Type::CharacterDevice
        } else if file_type.is_fifo() {
            file::Type::Fifo
        } else if file_type.is_socket() {
            file::Type::Socket
        } else {
            file::Type::Regular
        };

        file::Attr {
            file_type,
            mode: meta.mode(),
            nlink: meta.nlink() as u32,
            uid: meta.uid(),
            gid: meta.gid(),
            size: meta.size(),
            used: meta.blocks().saturating_mul(512),
            device: file::Device { major: 0, minor: 0 },
            fs_id: meta.dev(),
            file_id: meta.ino(),
            atime: Self::time_from_unix(meta.atime(), meta.atime_nsec()),
            mtime: Self::time_from_unix(meta.mtime(), meta.mtime_nsec()),
            ctime: Self::time_from_unix(meta.ctime(), meta.ctime_nsec()),
        }
    }

    fn wcc_attr_from_metadata(meta: &Metadata) -> file::WccAttr {
        file::WccAttr {
            size: meta.size(),
            mtime: Self::time_from_unix(meta.mtime(), meta.mtime_nsec()),
            ctime: Self::time_from_unix(meta.ctime(), meta.ctime_nsec()),
        }
    }

    fn metadata(path: &Path) -> Result<Metadata, vfs::Error> {
        std::fs::symlink_metadata(path).map_err(|error| Self::io_error_to_vfs(&error))
    }

    fn wcc_data(path: &Path, before: Option<file::WccAttr>) -> vfs::WccData {
        vfs::WccData {
            before,
            after: std::fs::symlink_metadata(path).ok().map(|meta| Self::attr_from_metadata(&meta)),
        }
    }

    fn validate_directory(attr: &file::Attr) -> Result<(), vfs::Error> {
        if matches!(attr.file_type, file::Type::Directory) {
            Ok(())
        } else {
            Err(vfs::Error::NotDir)
        }
    }

    fn validate_regular(attr: &file::Attr) -> Result<(), vfs::Error> {
        if matches!(attr.file_type, file::Type::Regular) {
            Ok(())
        } else {
            Err(vfs::Error::InvalidArgument)
        }
    }

    fn collect_slice_bytes(slice: &Slice, size: u32) -> Vec<u8> {
        let mut data = Vec::with_capacity(size as usize);
        for part in slice {
            data.extend_from_slice(part);
            if data.len() >= size as usize {
                data.truncate(size as usize);
                break;
            }
        }
        data
    }

    fn file_attr(path: &Path) -> Option<file::Attr> {
        std::fs::symlink_metadata(path).ok().map(|meta| Self::attr_from_metadata(&meta))
    }

    /// Stores an exclusive create verifier in the file's mtime (per RFC 1813 §3.3.8).
    fn store_exclusive_verifier(path: &Path, verifier: &[u8; NFS3_CREATEVERFSIZE]) {
        let sec = u32::from_be_bytes(verifier[0..4].try_into().unwrap());
        let nsec = u32::from_be_bytes(verifier[4..8].try_into().unwrap());
        let time = UNIX_EPOCH + Duration::new(u64::from(sec), nsec);
        let times = std::fs::FileTimes::new().set_modified(time);
        if let Ok(file) = std::fs::File::open(path) {
            let _ = file.set_times(times);
        }
    }

    /// Checks if an existing file's mtime matches the exclusive create verifier.
    fn check_exclusive_verifier(path: &Path, verifier: &[u8; NFS3_CREATEVERFSIZE]) -> bool {
        let Ok(meta) = std::fs::symlink_metadata(path) else { return false };
        let stored_sec = meta.mtime() as u32;
        let stored_nsec = meta.mtime_nsec() as u32;
        let expected_sec = u32::from_be_bytes(verifier[0..4].try_into().unwrap());
        let expected_nsec = u32::from_be_bytes(verifier[4..8].try_into().unwrap());
        stored_sec == expected_sec && stored_nsec == expected_nsec
    }

    fn apply_set_attr(path: &Path, new_attr: &set_attr::NewAttr) -> Result<(), vfs::Error> {
        if new_attr.uid.is_some() || new_attr.gid.is_some() {
            return Err(vfs::Error::InvalidArgument);
        }

        if let Some(mode) = new_attr.mode {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
                .map_err(|error| Self::io_error_to_vfs(&error))?;
        }

        if let Some(size) = new_attr.size {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .map_err(|error| Self::io_error_to_vfs(&error))?;
            file.set_len(size).map_err(|error| Self::io_error_to_vfs(&error))?;
        }

        let needs_atime = !matches!(new_attr.atime, set_attr::SetTime::DontChange);
        let needs_mtime = !matches!(new_attr.mtime, set_attr::SetTime::DontChange);
        if needs_atime || needs_mtime {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .open(path)
                .map_err(|error| Self::io_error_to_vfs(&error))?;
            let meta = file.metadata().map_err(|error| Self::io_error_to_vfs(&error))?;
            let current_attr = Self::attr_from_metadata(&meta);
            let atime = match new_attr.atime {
                set_attr::SetTime::DontChange => {
                    Self::system_time_from_file_time(current_attr.atime)
                }
                set_attr::SetTime::ToServer => SystemTime::now(),
                set_attr::SetTime::ToClient(time) => Self::system_time_from_file_time(time),
            };
            let mtime = match new_attr.mtime {
                set_attr::SetTime::DontChange => {
                    Self::system_time_from_file_time(current_attr.mtime)
                }
                set_attr::SetTime::ToServer => SystemTime::now(),
                set_attr::SetTime::ToClient(time) => Self::system_time_from_file_time(time),
            };
            let times = std::fs::FileTimes::new().set_accessed(atime).set_modified(mtime);
            file.set_times(times).map_err(|error| Self::io_error_to_vfs(&error))?;
        }

        Ok(())
    }

    fn list_directory_entries(
        &self,
        dir_path: &Path,
    ) -> Result<Vec<(file::Name, PathBuf, Metadata)>, vfs::Error> {
        let mut entries = Vec::new();
        let listing = std::fs::read_dir(dir_path).map_err(|error| Self::io_error_to_vfs(&error))?;

        for item in listing {
            let item = item.map_err(|error| Self::io_error_to_vfs(&error))?;
            let file_name = item.file_name();
            let name = file::Name::new(file_name.to_string_lossy().into_owned())
                .map_err(|_| vfs::Error::InvalidArgument)?;
            let path = item.path();
            let metadata = item.metadata().map_err(|error| Self::io_error_to_vfs(&error))?;
            entries.push((name, path, metadata));
        }

        entries.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));
        Ok(entries)
    }
}
