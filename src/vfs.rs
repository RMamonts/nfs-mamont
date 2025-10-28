//! Virtual File System trait definition for NFSv3 (RFC 1813).
//!
//! This module exposes a Rust-friendly interface that mirrors the
//! procedures described in the NFS version 3 specification. All types are
//! expressed using idiomatic Rust naming instead of the original C/XDR
//! definitions from the RFC.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

/// Result of [`Vfs`] operations.
pub type Result<T> = std::result::Result<T, NfsError>;

/// Maximum number of bytes allowed in a file handle (per RFC 1813 2.4).
pub const MAX_FILE_HANDLE_LEN: usize = 8;

/// Maximum number of bytes allowed in a file name (per RFC 1813 2.4).
pub const MAX_NAME_LEN: usize = 255;

/// Maximum length of path passed into [`Vfs`] methods.
pub const MAX_PATH_LEN: usize = 1024;

/// [`Vfs`] errors.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NfsError {
    // NFS3ERR_PERM
    Perm,
    // NFS3ERR_NOENT
    NoEnt,
    // NFS3ERR_IO
    Io,
    // NFS3ERR_NXIO
    NxIo,
    // NFS3ERR_ACCES
    Access,
    // NFS3ERR_EXIST
    Exist,
    // NFS3ERR_XDEV
    XDev,
    // NFS3ERR_NODEV
    Nodev,
    // NFS3ERR_NOTDIR
    NotDir,
    // NFS3ERR_ISDIR
    IsDir,
    // NFS3ERR_INVAL
    Inval,
    // NFS3ERR_FBIG
    FBig,
    // NFS3ERR_NOSPC
    NoSpc,
    // NFS3ERR_ROFS
    RoFs,
    // NFS3ERR_MLINK
    MLink,
    // NFS3ERR_NAME_TOO_LONG
    NameTooLong,
    // NFS3ERR_NOT_EMPTY
    NotEmpty,
    // NFS3ERR_DQUOT
    DQuot,
    // NFS3ERR_STALE
    Stale,
    // NFS3ERR_REMOTE
    Remote,
    // NFS3ERR_BAD_COOKIE
    BadCookie,
    // NFS3ERR_BADHANDLE
    BadHandle,
    // NFS3ERR_NOT_SYNC
    NotSync,
    // NFS3ERR_NOTSUPP
    NotSupp,
    // NFS3ERR_TOOSMALL
    TooSmall,
    // NFS3ERR_SERVERFAULT
    ServerFault,
    // NFS3ERR_BADTYPE
    BadType,
    // NFS3ERR_JUKEBOX
    Jukebox,
}

mod file {
    pub const UID_SIZE: usize = 8;

    /// Unique file identifier.
    /// 
    /// Corresponds to the file handle from RFC 1813.
    #[derive(Clone)]
    #[allow(dead_code)]
    #[allow(clippy::upper_case_acronyms)]
    pub struct UID(pub [u8; UID_SIZE]);

    /// File type.
    #[derive(Clone, Copy)]
    pub enum Type {
        Regular,
        Directory,
        BlockDevice,
        CharacterDevice,
        Symlink,
        Socket,
        Fifo,
    }

    /// File attributes.
    #[derive(Clone)]
    pub struct Attr {
        pub file_type: Type,
        pub mode: u32,
        pub nlink: u32,
        pub uid: u32,
        pub gid: u32,
        pub size: u64,
        pub used: u64,
        pub device: Option<Device>,
        pub fsid: u64,
        pub fileid: u64,
        pub atime: Time,
        pub mtime: Time,
        pub ctime: Time,
    }

    /// Time of file [`super::Vfs`] operations.
    #[derive(Copy, Clone)]
    pub struct Time {
        pub seconds: i64,
        pub nanos: u32,
    }

    /// Major and minor device pair.
    #[derive(Copy, Clone)]
    pub struct Device {
        pub major: u32,
        pub minor: u32,
    }
}

/// Digest used for weak cache consistency in [`WccData`].
#[derive(Copy, Clone)]
pub struct WccAttr {
    pub size: u64,
    pub mtime: file::Time,
    pub ctime: file::Time,
}

/// Weak cache consistency information (RFC 1813 3.1).
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

/// Attribute modifications (RFC 1813 3.3.2).
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
    pub file: file::UID,
    pub object_attr: file::Attr,
    pub directory_attr: Option<file::Attr>,
}

/// Mask of access rights (RFC 1813 3.3.4).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AccessMask(u32);

/// Result returned by [`Vfs::access`] (RFC 1813 3.3.4).
#[derive(Clone)]
pub struct AccessResult {
    pub granted: AccessMask,
    pub file_attr: Option<file::Attr>,
}

/// Data returned by [`Vfs::read`] (RFC 1813 3.3.6).
#[derive(Clone)]
pub struct ReadResult {
    pub data: Vec<u8>,
    pub file_attr: Option<file::Attr>,
}

/// Stability guarantee requested by [`Vfs::write`] (RFC 1813 3.3.7).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WriteMode {
    Unstable,
    DataSync,
    FileSync,
}

/// Stable write verifier (RFC 1813 3.3.7).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct StableVerifier(pub [u8; 8]);

/// Result returned by [`Vfs::write`] (RFC 1813 3.3.7).
#[derive(Clone)]
pub struct WriteResult {
    pub count: u32,
    pub committed: WriteMode,
    pub verifier: StableVerifier,
    pub file_attr: Option<file::Attr>,
}

/// Creation strategy (RFC 1813 3.3.8).
#[derive(Clone)]
pub enum CreateMode {
    Unchecked { attr: SetAttr },
    Guarded { attr: SetAttr, verifier: [u8; 8] },
    Exclusive { verifier: [u8; 8] },
}

/// Result returned by [`Vfs::create`] and similar operations.
#[derive(Clone)]
pub struct CreatedNode {
    pub file: file::UID,
    pub attr: file::Attr,
    pub directory_wcc: WccData,
}

/// Special node description used by [`Vfs::make_node`] (RFC 1813 3.3.11).
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

/// Result returned by [`Vfs::link`] (RFC 1813 3.3.15).
#[derive(Clone)]
pub struct LinkResult {
    pub new_file_attr: Option<file::Attr>,
    pub directory_wcc: WccData,
}

/// Result returned by [`Vfs::rename`] (RFC 1813 3.3.14).
#[derive(Clone)]
pub struct RenameResult {
    pub from_directory_wcc: WccData,
    pub to_directory_wcc: WccData,
}

/// Cookie used for directory iteration (RFC 1813 3.3.16).
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct DirectoryCookie(pub u64);

/// Cookie verifier (RFC 1813 3.3.16).
#[derive(Copy, Clone, PartialEq, Eq,)]
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
    pub file: Option<file::UID>,
    pub attr: Option<file::Attr>,
}

/// Result of [`Vfs::read_dir`] (RFC 1813 3.3.16).
#[derive(Clone)]
pub struct ReadDirResult {
    pub directory_attr: Option<file::Attr>,
    pub cookie_verifier: CookieVerifier,
    pub entries: Vec<DirectoryEntry>,
}

/// Result of [`Vfs::read_dir_plus`] (RFC 1813 3.3.17).
#[derive(Clone)]
pub struct ReadDirPlusResult {
    pub directory_attr: Option<file::Attr>,
    pub cookie_verifier: CookieVerifier,
    pub entries: Vec<DirectoryPlusEntry>,
}

/// Dynamic filesystem statistics (RFC 1813 3.3.18).
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

/// Static filesystem information (RFC 1813 3.3.19).
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

/// Filesystem capability flags (RFC 1813 3.3.19).
#[allow(dead_code)]
#[derive(Clone)]
pub struct FsProperties(u32);

/// POSIX path configuration information (RFC 1813 3.3.20).
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

/// Result returned by [`Vfs::commit`] (RFC 1813 3.3.21).
#[derive(Clone)]
pub struct CommitResult {
    pub file_attr: Option<file::Attr>,
    pub verifier: StableVerifier,
}

/// Virtual File System interface.
#[async_trait]
pub trait Vfs: Sync + Send {
    /// Get file attributes.
    async fn get_attr(&self, file: &file::UID) -> Result<file::Attr>;

    /// Set file attributes with optional guard.
    async fn set_attr(
        &self,
        file: &file::UID,
        attr: SetAttr,
        guard: Option<SetAttrGuard>,
    ) -> Result<WccData>;

    /// Lookup a name within a directory.
    async fn lookup(&self, parent: &file::UID, name: &str) -> Result<LookupResult>;

    /// Check requested access mask.
    async fn access(&self, file: &file::UID, mask: AccessMask) -> Result<AccessResult>;

    /// Read symbolic link contents.
    async fn read_link(&self, file: &file::UID) -> Result<(PathBuf, Option<file::Attr>)>;

    /// Read file data.
    async fn read(&self, file: &file::UID, offset: u64, count: u32) -> Result<ReadResult>;

    /// Write file data with stability mode.
    async fn write(
        &self,
        file: &file::UID,
        offset: u64,
        data: &[u8],
        mode: WriteMode,
    ) -> Result<WriteResult>;

    /// Create and optionally initialize a regular file.
    async fn create(&self, parent: &file::UID, name: &str, mode: CreateMode)
        -> Result<CreatedNode>;

    /// Create a directory.
    async fn make_dir(&self, parent: &file::UID, name: &str, attr: SetAttr) -> Result<CreatedNode>;

    /// Create a symbolic link.
    async fn make_symlink(
        &self,
        parent: &file::UID,
        name: &str,
        target: &Path,
        attr: SetAttr,
    ) -> Result<CreatedNode>;

    /// Create a special node.
    async fn make_node(
        &self,
        parent: &file::UID,
        name: &str,
        node: SpecialNode,
    ) -> Result<CreatedNode>;

    /// Delete a file.
    async fn remove(&self, parent: &file::UID, name: &str) -> Result<RemovalResult>;

    /// Delete an empty directory.
    async fn remove_dir(&self, parent: &file::UID, name: &str) -> Result<RemovalResult>;

    /// Atomically move a file or directory.
    async fn rename(
        &self,
        from_parent: &file::UID,
        from_name: &str,
        to_parent: &file::UID,
        to_name: &str,
    ) -> Result<RenameResult>;

    /// Create a hard link.
    async fn link(
        &self,
        source: &file::UID,
        new_parent: &file::UID,
        new_name: &str,
    ) -> Result<LinkResult>;

    /// Iterate directory entries.
    async fn read_dir(
        &self,
        file: &file::UID,
        cookie: DirectoryCookie,
        verifier: CookieVerifier,
        max_bytes: u32,
    ) -> Result<ReadDirResult>;

    /// Iterate directory entries with attributes.
    async fn read_dir_plus(
        &self,
        file: &file::UID,
        cookie: DirectoryCookie,
        verifier: CookieVerifier,
        max_bytes: u32,
        max_filess: u32,
    ) -> Result<ReadDirPlusResult>;

    /// Get dynamic filesystem statistics.
    async fn fs_stat(&self, file: &file::UID) -> Result<FsStat>;

    /// Get static filesystem information.
    async fn fs_info(&self, file: &file::UID) -> Result<FsInfo>;

    /// Get POSIX path capabilities.
    async fn path_conf(&self, file: &file::UID) -> Result<PathConfig>;

    /// Commit previous writes to stable storage.
    async fn commit(&self, file: &file::UID, offset: u64, count: u32) -> Result<CommitResult>;
}
