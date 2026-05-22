use std::fs::Metadata;
use std::io::ErrorKind;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_lock::RwLock;

use libc;
use nfs_mamont::consts::nfsv3::{NFS3_COOKIEVERFSIZE, NFS3_CREATEVERFSIZE};
use nfs_mamont::vfs;
use nfs_mamont::vfs::file;
use nfs_mamont::vfs::read_dir;
use nfs_mamont::vfs::set_attr;
use nfs_mamont::vfs::write;

use crate::fs_map::FsMap;
use crate::uring;

/// RAII guard for raw file descriptors
struct FdGuard(RawFd);

impl FdGuard {
    fn new(fd: RawFd) -> Self {
        Self(fd)
    }

    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl Drop for FdGuard {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe {
                libc::close(self.0);
            }
        }
    }
}

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
    fsmap: RwLock<FsMap>,
    generation: u64,
    uring: Option<Arc<uring::UringPool>>,
}

impl MirrorFS {
    /// Creates a new mirror file system with the given root path.
    pub fn new(root: PathBuf, ring_count: usize, ring_size: u32) -> Self {
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        let generation =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_nanos()
                as u64;
        let uring = uring::UringPool::new(ring_count, ring_size);
        Self { fsmap: RwLock::new(FsMap::new(root)), generation, uring }
    }

    /// Returns the root handle.
    pub async fn root_handle(&self) -> file::Handle {
        self.fsmap.read().await.root_handle()
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

    async fn path_for_handle(&self, handle: &file::Handle) -> Result<PathBuf, vfs::Error> {
        let fsmap = self.fsmap.read().await;
        fsmap.path_for_handle(handle)
    }
    /// Returns a handle for a path under the mirror root.
    pub async fn handle_for_path(&self, path: &Path) -> Result<file::Handle, vfs::Error> {
        self.fsmap.write().await.ensure_handle_for_path(path)
    }

    async fn remove_cached_path(&self, path: &Path) {
        self.fsmap.write().await.remove_path(path);
    }

    async fn rename_cached_path(&self, from: &Path, to: &Path) -> Result<(), vfs::Error> {
        self.fsmap.write().await.rename_path(from, to)
    }

    fn ensure_name_allowed(name: &file::Name) -> Result<(), vfs::Error> {
        match name.as_str() {
            "." => Err(vfs::Error::InvalidArgument),
            ".." => Err(vfs::Error::Exist),
            _ => Ok(()),
        }
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

    fn attr_from_statx(meta: &uring::StatxData) -> file::Attr {
        let file_type = match meta.mode & libc::S_IFMT {
            libc::S_IFDIR => file::Type::Directory,
            libc::S_IFLNK => file::Type::Symlink,
            libc::S_IFREG => file::Type::Regular,
            libc::S_IFBLK => file::Type::BlockDevice,
            libc::S_IFCHR => file::Type::CharacterDevice,
            libc::S_IFIFO => file::Type::Fifo,
            libc::S_IFSOCK => file::Type::Socket,
            _ => file::Type::Regular,
        };
        let fs_id = ((meta.dev_major as u64) << 32) | meta.dev_minor as u64;

        file::Attr {
            file_type,
            mode: meta.mode,
            nlink: meta.nlink,
            uid: meta.uid,
            gid: meta.gid,
            size: meta.size,
            used: meta.blocks.saturating_mul(512),
            device: file::Device { major: 0, minor: 0 },
            fs_id,
            file_id: meta.ino,
            atime: Self::time_from_unix(meta.atime_sec, meta.atime_nsec),
            mtime: Self::time_from_unix(meta.mtime_sec, meta.mtime_nsec),
            ctime: Self::time_from_unix(meta.ctime_sec, meta.ctime_nsec),
        }
    }

    fn wcc_attr_from_statx(meta: &uring::StatxData) -> file::WccAttr {
        file::WccAttr {
            size: meta.size,
            mtime: Self::time_from_unix(meta.mtime_sec, meta.mtime_nsec),
            ctime: Self::time_from_unix(meta.ctime_sec, meta.ctime_nsec),
        }
    }

    async fn metadata(&self, path: &Path) -> Result<uring::StatxData, vfs::Error> {
        if let Some(uring) = self.uring_executor() {
            return uring.statx(path, false).await.map_err(|error| Self::io_error_to_vfs(&error));
        }

        let meta =
            std::fs::symlink_metadata(path).map_err(|error| Self::io_error_to_vfs(&error))?;
        Ok(Self::statx_from_metadata(&meta))
    }

    async fn wcc_data(&self, path: &Path, before: Option<file::WccAttr>) -> vfs::WccData {
        let after = self.metadata(path).await.ok().map(|meta| Self::attr_from_statx(&meta));
        vfs::WccData { before, after }
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

    async fn write_all_uring(
        &self,
        fd: RawFd,
        mut offset: u64,
        data: &[u8],
    ) -> Result<(), std::io::Error> {
        let Some(uring) = self.uring_executor() else {
            return Err(std::io::Error::other("io_uring not available"));
        };

        let mut written = 0usize;
        let max_len = self.uring_max_io_len();
        while written < data.len() {
            let end = (written + max_len).min(data.len());
            let chunk = data[written..end].to_vec();
            let bytes = uring.write_at(fd, offset, chunk).await?;
            if bytes == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "io_uring write returned 0 bytes",
                ));
            }
            written += bytes;
            offset += bytes as u64;
        }

        Ok(())
    }

    async fn read_at_uring(
        &self,
        fd: RawFd,
        offset: u64,
        len: usize,
    ) -> Result<Vec<u8>, std::io::Error> {
        let Some(uring) = self.uring_executor() else {
            return Err(std::io::Error::other("io_uring not available"));
        };

        if len == 0 {
            return Ok(Vec::new());
        }

        let max_len = self.uring_max_io_len();
        if len <= max_len {
            return uring.read_at(fd, offset, len).await;
        }

        let mut buffer = Vec::with_capacity(len);
        let mut remaining = len;
        let mut current_offset = offset;
        while remaining > 0 {
            let chunk_len = remaining.min(max_len);
            let chunk = uring.read_at(fd, current_offset, chunk_len).await?;
            if chunk.is_empty() {
                break;
            }
            current_offset += chunk.len() as u64;
            remaining = remaining.saturating_sub(chunk.len());
            buffer.extend_from_slice(&chunk);
            if chunk.len() < chunk_len {
                break;
            }
        }

        Ok(buffer)
    }

    async fn file_attr(&self, path: &Path) -> Option<file::Attr> {
        self.metadata(path).await.ok().map(|meta| Self::attr_from_statx(&meta))
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

    async fn list_directory_entries(
        &self,
        dir_path: &Path,
    ) -> Result<Vec<(file::Name, PathBuf, uring::StatxData)>, vfs::Error> {
        let mut entries = Vec::new();
        let listing = std::fs::read_dir(dir_path).map_err(|error| Self::io_error_to_vfs(&error))?;

        for item in listing {
            let item = item.map_err(|error| Self::io_error_to_vfs(&error))?;
            let file_name = item.file_name();
            let name = file::Name::new(file_name.to_string_lossy().into_owned())
                .map_err(|_| vfs::Error::InvalidArgument)?;
            let path = item.path();
            let metadata = self.metadata(&path).await?;
            entries.push((name, path, metadata));
        }

        entries.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));
        Ok(entries)
    }

    fn statx_from_metadata(meta: &Metadata) -> uring::StatxData {
        uring::StatxData {
            mode: meta.mode(),
            nlink: meta.nlink() as u32,
            uid: meta.uid(),
            gid: meta.gid(),
            size: meta.size(),
            blocks: meta.blocks(),
            dev_major: 0,
            dev_minor: 0,
            ino: meta.ino(),
            atime_sec: meta.atime(),
            atime_nsec: meta.atime_nsec(),
            mtime_sec: meta.mtime(),
            mtime_nsec: meta.mtime_nsec(),
            ctime_sec: meta.ctime(),
            ctime_nsec: meta.ctime_nsec(),
        }
    }

    async fn open_fd_uring(
        &self,
        path: &Path,
        flags: i32,
        mode: u32,
    ) -> Result<FdGuard, std::io::Error> {
        let Some(uring) = self.uring_executor() else {
            return Err(std::io::Error::other("io_uring not available"));
        };

        let fd = uring.open_at(path, flags, mode).await?;
        Ok(FdGuard::new(fd))
    }

    fn uring_executor(&self) -> Option<Arc<uring::UringExecutor>> {
        self.uring.as_ref().map(|pool| pool.pick())
    }

    fn uring_max_io_len(&self) -> usize {
        self.uring.as_ref().map(|pool| pool.max_io_len()).unwrap_or(usize::MAX)
    }

    async fn child_path(
        &self,
        dir: &file::Handle,
        name: &file::Name,
    ) -> Result<PathBuf, vfs::Error> {
        let dir_path = self.path_for_handle(dir).await?;
        let mut child = dir_path;
        child.push(name.as_str());
        Ok(child)
    }

    async fn exported_root_path(&self) -> Result<PathBuf, vfs::Error> {
        let root = self.root_handle().await;
        self.path_for_handle(&root).await
    }
}
