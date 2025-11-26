//! Defines NFSv3 Virtual File System interface --- [`Vfs`].

mod file;

use std::path::{Path, PathBuf};

use async_trait::async_trait;

/// Result of [`Vfs`] operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Maximum length of names passed into [`Vfs`] methods.
pub const MAX_NAME_LEN: usize = 255;

/// Maximum length of file paths passed into [`Vfs`] methods.
pub const MAX_PATH_LEN: usize = 1024;

/// [`Vfs`] errors.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not owner. The operation was not allowed because the
    /// caller is either not a privileged user (root) or not the
    /// owner of the target of the operation.
    Permission,
    // Assume NOENT.
    /// No such file or directory. The file or directory name
    /// specified does not exist.
    NoEntry,
    /// I/O error. A hard error (for example, a disk error)
    /// occurred while processing the requested operation.
    IO,
    /// I/O error. No such device or address.
    NXIO,
    /// Permission denied. The caller does not have the correct
    /// permission to perform the requested operation. Contrast
    /// this with NFS3ERR_PERM, which restricts itself to owner
    /// or privileged user permission failures.
    Access,
    /// File exists. The file specified already exists.
    Exist,
    /// Attempt to do a cross-device hard link.
    XDev,
    /// No such device.
    NoDev,
    /// Not a directory. The caller specified a non-directory in
    /// a directory operation.
    NotDir,
    /// Is a directory. The caller specified a directory in a
    /// non-directory operation.
    IsDir,
    /// Invalid argument or unsupported argument for an
    /// operation. Two examples are attempting a [`Vfs::read_link`] on an
    /// object other than a symbolic link or attempting to
    /// [`Vfs::set_attr`] a time field on a server that does not support
    /// this operation.
    InvalidArgument,
    /// File too large. The operation would have caused a file to
    /// grow beyond the server's limit.
    FileTooLarge,
    /// No space left on device. The operation would have caused
    /// the server's file system to exceed its limit.
    NoSpace,
    /// Read-only file system. A modifying operation was
    /// attempted on a read-only file system.
    ReadOnlyFs,
    /// Too many hard links.
    TooManyLinks,
    /// The filename in an operation was too long.
    NameTooLong,
    /// An attempt was made to remove a directory that was not
    /// empty.
    NotEmpty,
    /// Resource (quota) hard limit exceeded. The user's resource
    /// limit on the server has been exceeded.
    QuotaExceeded,
    /// Invalid file handle. The file handle given in the
    /// arguments was invalid. The file referred to by that file
    /// handle no longer exists or access to it has been
    /// revoked.
    StaleFile,
    /// Too many levels of remote in path. The file handle given
    /// in the arguments referred to a file on a non-local file
    /// system on the server.
    TooManyLevelsOfRemote,
    /// Illegal NFS file handle. The file handle failed internal
    /// consistency checks.
    BadFileHandle,
    /// Update synchronization mismatch was detected during a
    /// [`Vfs::set_attr`] operation.
    NotSync,
    /// [`Vfs::read_dir`] or [`Vfs::read_dir_plus`] cookie is stale.
    BadCookie,
    /// Operation is not supported.
    NotSupp,
    /// Buffer or request is too small.
    TooSmall,
    /// An error occurred on the server which does not map to any
    /// of the legal NFS version 3 protocol error values.  The
    /// client should translate this into an appropriate error.
    /// UNIX clients may choose to translate this to EIO.
    ServerFault,
    /// An attempt was made to create an object of a type not
    /// supported by the [`Vfs`] implementation.
    BadType,
    /// The server initiated the request, but was not able to
    /// complete it in a timely fashion. The client should wait
    /// and then try the request with a new RPC transaction ID.
    /// For example, this error should be returned from a server
    /// that supports hierarchical storage and receives a request
    /// to process a file that has been migrated. In this case,
    /// the server should start the immigration process and
    /// respond to client with this error.
    JUKEBOX
}

/// Weak cache consistency attributes.
#[derive(Copy, Clone)]
pub struct WccAttr {
    pub size: u64,
    pub mtime: file::Time,
    pub ctime: file::Time,
}

/// Weak cache consistency information.
#[derive(Clone)]
pub struct WccData {
    pub before: Option<WccAttr>,
    pub after: Option<file::Attr>,
}

/// Strategy for updating timestamps in [`SetAttr`].
#[derive(Copy, Clone)]
pub enum SetTime {
    DontChange,
    ServerCurrent,
    ClientProvided(file::Time),
}

/// Attribute modification.
#[derive(Clone)]
pub struct SetAttr {
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub size: Option<u64>,
    pub atime: SetTime,
    pub mtime: SetTime,
}

/// Guard used by [`Vfs::set_attr`] to enforce weak cache consistency.
#[derive(Copy, Clone)]
#[allow(dead_code)]
pub struct SetAttrGuard {
    ctime: file::Time,
}

/// Result of a [`Vfs::lookup`] operation (RFC 1813 3.3.3).
#[derive(Clone)]
pub struct LookupResult {
    pub file: file::Uid,
    pub object_attr: file::Attr,
    pub directory_attr: Option<file::Attr>,
}

/// Mask of access rights.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AccessMask(u32);

/// Result returned by [`Vfs::access`].
#[derive(Clone)]
pub struct AccessResult {
    pub granted: AccessMask,
    pub file_attr: Option<file::Attr>,
}

/// Data returned by [`Vfs::read`].
#[derive(Clone)]
pub struct ReadResult {
    pub data: Vec<u8>,
    pub file_attr: Option<file::Attr>,
}

/// Stability guarantee requested by [`Vfs::write`].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WriteMode {
    Unstable,
    DataSync,
    FileSync,
}

/// Stable write verifier.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct StableVerifier(pub [u8; 8]);

/// Result returned by [`Vfs::write`].
#[derive(Clone)]
pub struct WriteResult {
    pub count: u32,
    pub committed: WriteMode,
    pub verifier: StableVerifier,
    pub file_attr: Option<file::Attr>,
}

/// Creation strategy.
#[derive(Clone)]
pub enum CreateMode {
    Unchecked { attr: SetAttr },
    Guarded { attr: SetAttr, verifier: [u8; 8] },
    Exclusive { verifier: [u8; 8] },
}

/// Result returned by [`Vfs::create`] and similar operations.
#[derive(Clone)]
pub struct CreatedNode {
    pub file: file::Uid,
    pub attr: file::Attr,
    pub directory_wcc: WccData,
}

/// Special node description used by [`Vfs::make_node`].
#[derive(Clone)]
pub enum SpecialNode {
    Block { device: file::Device, attr: SetAttr },
    Character { device: file::Device, attr: SetAttr },
    Socket { attr: SetAttr },
    Fifo { attr: SetAttr },
}

/// Result returned by [`Vfs::remove`] and [`Vfs::remove_dir`] operations.
#[derive(Clone)]
pub struct RemovalResult {
    pub directory_wcc: WccData,
}

/// Result returned by [`Vfs::link`].
#[derive(Clone)]
pub struct LinkResult {
    pub new_file_attr: Option<file::Attr>,
    pub directory_wcc: WccData,
}

/// Result returned by [`Vfs::rename`].
#[derive(Clone)]
pub struct RenameResult {
    pub from_directory_wcc: WccData,
    pub to_directory_wcc: WccData,
}

/// Cookie used for directory iteration.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct DirectoryCookie(pub u64);

/// Cookie verifier.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct CookieVerifier(pub [u8; 8]);

/// Minimal directory entry returned by [`Vfs::read_dir`].
#[derive(Clone, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub cookie: DirectoryCookie,
    pub name: String,
    pub fileid: u64,
}

/// Extended directory entry returned by [`Vfs::read_dir_plus`].
#[derive(Clone)]
pub struct DirectoryPlusEntry {
    pub cookie: DirectoryCookie,
    pub name: String,
    pub fileid: u64,
    pub file: Option<file::Uid>,
    pub attr: Option<file::Attr>,
}

/// Result of [`Vfs::read_dir`].
#[derive(Clone)]
pub struct ReadDirResult {
    pub directory_attr: Option<file::Attr>,
    pub cookie_verifier: CookieVerifier,
    pub entries: Vec<DirectoryEntry>,
}

/// Result of [`Vfs::read_dir_plus`].
#[derive(Clone)]
pub struct ReadDirPlusResult {
    pub directory_attr: Option<file::Attr>,
    pub cookie_verifier: CookieVerifier,
    pub entries: Vec<DirectoryPlusEntry>,
}

/// Dynamic filesystem statistics.
#[derive(Clone)]
pub struct FsStat {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub total_files: u64,
    pub free_files: u64,
    pub available_files: u64,
    pub invarsec: u32,
    pub file_attr: Option<file::Attr>,
}

/// Static filesystem information.
#[derive(Clone)]
pub struct FsInfo {
    pub read_max: u32,
    pub read_pref: u32,
    pub read_multiple: u32,
    pub write_max: u32,
    pub write_pref: u32,
    pub write_multiple: u32,
    pub directory_pref: u32,
    pub max_file_size: u64,
    pub time_delta: file::Time,
    pub properties: FsProperties,
    pub file_attr: Option<file::Attr>,
}

/// Filesystem capability flags.
#[allow(dead_code)]
#[derive(Clone)]
pub struct FsProperties(u32);

/// POSIX path configuration information.
#[derive(Clone)]
pub struct PathConfig {
    pub file_attr: Option<file::Attr>,
    pub max_link: u32,
    pub max_name: u32,
    pub no_trunc: bool,
    pub chown_restricted: bool,
    pub case_insensitive: bool,
    pub case_preserving: bool,
}

/// Result returned by [`Vfs::commit`].
#[derive(Clone)]
pub struct CommitResult {
    pub file_attr: Option<file::Attr>,
    pub verifier: StableVerifier,
}

pub mod promise {
    use super::*;

    pub trait GetAttr {
        fn keep(self, promise: Result<file::Attr>);
    }

    pub trait SetAttr {
        fn keep(self, promise: Result<WccData>);
    }

    pub trait Lookup {
        fn keep(self, promise: Result<LookupResult>);
    }

    pub trait Access {
        fn keep(self, promise: Result<AccessResult>);
    }

    pub trait ReadLink {
        fn keep(self, promise: Result<(PathBuf, Option<file::Attr>)>);
    }

    pub trait Read {
        fn keep(self, promise: Result<ReadResult>);
    }

    pub trait Write {
        fn keep(self, promise: Result<WriteResult>);
    }

    pub trait Create {
        fn keep(self, promise: Result<CreatedNode>);
    }

    pub trait MakeDir {
        fn keep(self, promise: Result<CreatedNode>);
    }

    pub trait MakeSymlink {
        fn keep(self, promise: Result<CreatedNode>);
    }

    pub trait MakeNode {
        fn keep(self, promise: Result<CreatedNode>);
    }

    pub trait Remove {
        fn keep(self, promise: Result<RemovalResult>);
    }

    pub trait RemoveDir {
        fn keep(self, promise: Result<RemovalResult>);
    }

    pub trait Rename {
        fn keep(self, promise: Result<RenameResult>);
    }

    pub trait Link {
        fn keep(self, promise: Result<LinkResult>);
    }

    pub trait ReadDir {
        fn keep(self, promise: Result<ReadDirResult>);
    }

    pub trait ReadDirPlus {
        fn keep(self, promise: Result<ReadDirPlusResult>);
    }

    pub trait FsStat {
        fn keep(self, promise: Result<super::FsStat>);
    }

    pub trait FsInof {
        fn keep(self, promise: Result<FsInfo>);
    }

    pub trait PathConf {
        fn keep(self, promise: Result<PathConfig>);
    }

    pub trait Commit {
        fn keep(self, promise: Result<CommitResult>);
    }
}

/// Virtual File System interface.
#[async_trait]
pub trait Vfs: Sync + Send {
    async fn get_attr(&self, file: &file::Uid);

    async fn set_attr(&self, file: &file::Uid, attr: SetAttr, guard: Option<SetAttrGuard>);

    async fn lookup(&self, parent: &file::Uid, name: &str);

    async fn access(&self, file: &file::Uid, mask: AccessMask);

    async fn read_link(&self, file: &file::Uid);

    async fn read(&self, file: &file::Uid, offset: u64, count: u32);

    async fn write(&self, file: &file::Uid, offset: u64, data: &[u8], mode: WriteMode);

    async fn create(&self, parent: &file::Uid, name: &str, mode: CreateMode);

    async fn make_dir(&self, parent: &file::Uid, name: &str, attr: SetAttr);

    async fn make_symlink(&self, parent: &file::Uid, name: &str, target: &Path, attr: SetAttr);

    async fn make_node(&self, parent: &file::Uid, name: &str, node: SpecialNode);

    async fn remove(&self, parent: &file::Uid, name: &str);

    async fn remove_dir(&self, parent: &file::Uid, name: &str);

    async fn rename(
        &self,
        from_parent: &file::Uid,
        from_name: &str,
        to_parent: &file::Uid,
        to_name: &str,
    );

    async fn link(&self, source: &file::Uid, new_parent: &file::Uid, new_name: &str);

    async fn read_dir(
        &self,
        file: &file::Uid,
        cookie: DirectoryCookie,
        verifier: CookieVerifier,
        max_bytes: u32,
    );

    async fn read_dir_plus(
        &self,
        file: &file::Uid,
        cookie: DirectoryCookie,
        verifier: CookieVerifier,
        max_bytes: u32,
        max_files: u32,
    );

    async fn fs_stat(&self, file: &file::Uid);

    async fn fs_info(&self, file: &file::Uid);

    async fn path_conf(&self, file: &file::Uid);

    async fn commit(&self, file: &file::Uid, offset: u64, count: u32);
}
