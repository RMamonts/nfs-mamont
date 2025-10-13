//! Virtual File System trait definition for NFSv3 (RFC 1813).
//!
//! This module exposes a Rust-friendly interface that mirrors the
//! procedures described in the NFS version 3 specification. All types are
//! expressed using idiomatic Rust naming instead of the original C/XDR
//! definitions from the RFC.

use async_trait::async_trait;
use std::ops::{BitAnd, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

/// Convenient result alias used by all VFS operations.
pub type VfsResult<T> = Result<T, NfsError>;

/// Maximum number of bytes allowed in a file handle (per RFC 1813 2.4).
pub const MAX_FILE_HANDLE_LEN: usize = 64;

/// Maximum number of bytes allowed in a file name (per RFC 1813 2.4).
pub const MAX_NAME_LEN: usize = 255;

/// Maximum number of bytes allowed in a file path (per RFC 1813 2.4).
pub const MAX_PATH_LEN: usize = 1024;

/// NFSv3 status codes (RFC 1813 2.6).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
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

/// Handle that uniquely identifies an inode inside the exported filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileHandle(pub Vec<u8>);

impl FileHandle {
    pub fn new(raw: Vec<u8>) -> Self {
        assert!(raw.len() <= MAX_FILE_HANDLE_LEN, "file handle exceeds RFC limit");
        Self(raw)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Canonical representation of a filesystem path according to RFC limits.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FsPath(pub String);

impl FsPath {
    pub fn new(path: impl Into<String>) -> Self {
        let value = path.into();
        assert!(value.len() <= MAX_PATH_LEN, "path exceeds RFC limit");
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for FsPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// File or directory name that respects RFC limits.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileName(pub String);

impl FileName {
    pub fn new(name: impl Into<String>) -> Self {
        let value = name.into();
        assert!(value.len() <= MAX_NAME_LEN, "file name exceeds RFC limit");
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for FileName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// POSIX-like file types enumerated in RFC 1813 3.3.1.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum FileType {
    Regular,
    Directory,
    BlockDevice,
    CharacterDevice,
    Symlink,
    Socket,
    Fifo,
}

/// Representation of a major/minor device pair.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct DeviceId {
    pub major: u32,
    pub minor: u32,
}

/// Timestamp structure matching `nfstime3`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct FileTime {
    pub seconds: i64,
    pub nanos: u32,
}

impl FileTime {
    pub const fn new(seconds: i64, nanos: u32) -> Self {
        Self { seconds, nanos }
    }
}

/// Full file attributes (RFC 1813 3.3.1).
#[derive(Debug, Clone, PartialEq)]
pub struct FileAttr {
    pub file_type: FileType,
    pub mode: u32,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub used: u64,
    pub device: Option<DeviceId>,
    pub fsid: u64,
    pub fileid: u64,
    pub atime: FileTime,
    pub mtime: FileTime,
    pub ctime: FileTime,
}

/// Digest used for weak cache consistency (size, mtime, ctime).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct AttrDigest {
    pub size: u64,
    pub mtime: FileTime,
    pub ctime: FileTime,
}

impl From<&FileAttr> for AttrDigest {
    fn from(value: &FileAttr) -> Self {
        Self { size: value.size, mtime: value.mtime, ctime: value.ctime }
    }
}

/// Weak cache consistency information (RFC 1813 3.1).
#[derive(Debug, Clone, PartialEq)]
pub struct WccData {
    pub before: Option<AttrDigest>,
    pub after: Option<FileAttr>,
}

impl WccData {
    pub fn empty() -> Self {
        Self { before: None, after: None }
    }
}

/// Strategy for updating timestamps in [`SetAttr`].
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SetTime {
    #[default]
    DontChange,
    ServerCurrent,
    ClientProvided(FileTime),
}

/// Attribute modifications (RFC 1813 3.3.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SetAttr {
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub size: Option<u64>,
    pub atime: SetTime,
    pub mtime: SetTime,
}

impl Default for SetAttr {
    fn default() -> Self {
        Self {
            mode: None,
            uid: None,
            gid: None,
            size: None,
            atime: SetTime::DontChange,
            mtime: SetTime::DontChange,
        }
    }
}

/// Guard used by [`Vfs::set_attr`] to enforce weak cache consistency.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SetAttrGuard {
    None,
    Check { ctime: FileTime },
}

/// Result of a LOOKUP operation (RFC 1813 3.3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct LookupResult {
    pub handle: FileHandle,
    pub object_attr: FileAttr,
    pub directory_attr: Option<FileAttr>,
}

/// Mask of access rights (RFC 1813 3.3.4).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct AccessMask(u32);

impl AccessMask {
    pub const READ: AccessMask = AccessMask(0x0001);
    pub const LOOKUP: AccessMask = AccessMask(0x0002);
    pub const MODIFY: AccessMask = AccessMask(0x0004);
    pub const EXTEND: AccessMask = AccessMask(0x0008);
    pub const DELETE: AccessMask = AccessMask(0x0010);
    pub const EXECUTE: AccessMask = AccessMask(0x0020);

    /// Creates an empty mask.
    pub const fn empty() -> Self {
        AccessMask(0)
    }

    /// Returns raw bit representation.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Checks whether all bits from `other` are present.
    pub const fn contains(self, other: AccessMask) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl BitOr for AccessMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        AccessMask(self.0 | rhs.0)
    }
}

impl BitOrAssign for AccessMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for AccessMask {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        AccessMask(self.0 & rhs.0)
    }
}

impl BitXor for AccessMask {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        AccessMask(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for AccessMask {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for AccessMask {
    type Output = Self;

    fn not(self) -> Self::Output {
        AccessMask(!self.0)
    }
}

/// Result returned by ACCESS (RFC 1813 3.3.4).
#[derive(Debug, Clone, PartialEq)]
pub struct AccessResult {
    pub granted: AccessMask,
    pub file_attr: Option<FileAttr>,
}

/// Target path stored in a symbolic link (RFC 1813 3.3.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymlinkTarget(pub String);

/// Data returned by READ (RFC 1813 3.3.6).
#[derive(Debug, Clone, PartialEq)]
pub struct ReadResult {
    pub data: Vec<u8>,
    pub eof: bool,
    pub file_attr: Option<FileAttr>,
}

/// Stability guarantee requested by WRITE (RFC 1813 3.3.7).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum WriteMode {
    Unstable,
    DataSync,
    FileSync,
}

/// Stable write verifier (RFC 1813 3.3.7).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct StableVerifier(pub [u8; 8]);

/// Result returned by WRITE (RFC 1813 3.3.7).
#[derive(Debug, Clone, PartialEq)]
pub struct WriteResult {
    pub count: u32,
    pub committed: WriteMode,
    pub verifier: StableVerifier,
    pub file_attr: Option<FileAttr>,
}

/// Creation strategy (RFC 1813 3.3.8).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CreateMode {
    Unchecked { attr: SetAttr },
    Guarded { attr: SetAttr, verifier: [u8; 8] },
    Exclusive { verifier: [u8; 8] },
}

/// Result returned by CREATE and similar operations.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatedNode {
    pub handle: FileHandle,
    pub attr: FileAttr,
    pub directory_wcc: WccData,
}

/// Special node description used by MKNOD (RFC 1813 3.3.11).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpecialNode {
    Block { device: DeviceId, attr: SetAttr },
    Character { device: DeviceId, attr: SetAttr },
    Socket { attr: SetAttr },
    Fifo { attr: SetAttr },
}

/// Result returned by REMOVE and RMDIR operations.
#[derive(Debug, Clone, PartialEq)]
pub struct RemovalResult {
    pub directory_wcc: WccData,
}

/// Result returned by LINK (RFC 1813 3.3.15).
#[derive(Debug, Clone, PartialEq)]
pub struct LinkResult {
    pub new_file_attr: Option<FileAttr>,
    pub directory_wcc: WccData,
}

/// Result returned by RENAME (RFC 1813 3.3.14).
#[derive(Debug, Clone, PartialEq)]
pub struct RenameResult {
    pub from_directory_wcc: WccData,
    pub to_directory_wcc: WccData,
}

/// Cookie used for directory iteration (RFC 1813 3.3.16).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct DirectoryCookie(pub u64);

/// Cookie verifier (RFC 1813 3.3.16).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct CookieVerifier(pub [u8; 8]);

/// Minimal directory entry returned by READDIR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DirectoryEntry {
    pub cookie: DirectoryCookie,
    pub name: FileName,
    pub fileid: u64,
}

/// Extended directory entry returned by READDIRPLUS.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectoryPlusEntry {
    pub cookie: DirectoryCookie,
    pub name: FileName,
    pub fileid: u64,
    pub handle: Option<FileHandle>,
    pub attr: Option<FileAttr>,
}

/// Result of READDIR (RFC 1813 3.3.16).
#[derive(Debug, Clone, PartialEq)]
pub struct ReadDirResult {
    pub directory_attr: Option<FileAttr>,
    pub cookie_verifier: CookieVerifier,
    pub entries: Vec<DirectoryEntry>,
    pub eof: bool,
}

/// Result of READDIRPLUS (RFC 1813 3.3.17).
#[derive(Debug, Clone, PartialEq)]
pub struct ReadDirPlusResult {
    pub directory_attr: Option<FileAttr>,
    pub cookie_verifier: CookieVerifier,
    pub entries: Vec<DirectoryPlusEntry>,
    pub eof: bool,
}

/// Dynamic filesystem statistics (RFC 1813 3.3.18).
#[derive(Debug, Clone, PartialEq)]
pub struct FsStat {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub total_files: u64,
    pub free_files: u64,
    pub available_files: u64,
    pub invarsec: u32,
    pub file_attr: Option<FileAttr>,
}

/// Static filesystem information (RFC 1813 3.3.19).
#[derive(Debug, Clone, PartialEq)]
pub struct FsInfo {
    pub read_max: u32,
    pub read_pref: u32,
    pub read_multiple: u32,
    pub write_max: u32,
    pub write_pref: u32,
    pub write_multiple: u32,
    pub directory_pref: u32,
    pub max_file_size: u64,
    pub time_delta: FileTime,
    pub properties: FsProperties,
    pub file_attr: Option<FileAttr>,
}

/// Filesystem capability flags (RFC 1813 3.3.19).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct FsProperties(u32);

impl FsProperties {
    pub const HARD_LINK: FsProperties = FsProperties(0x0000_0001);
    pub const SYMLINK: FsProperties = FsProperties(0x0000_0002);
    pub const HOMOGENEOUS: FsProperties = FsProperties(0x0000_0008);
    pub const CAN_SET_TIME: FsProperties = FsProperties(0x0000_0010);

    /// Returns an empty property mask.
    pub const fn empty() -> Self {
        FsProperties(0)
    }

    /// Returns raw property bits.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Checks if all bits in `other` are set.
    pub const fn contains(self, other: FsProperties) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl BitOr for FsProperties {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        FsProperties(self.0 | rhs.0)
    }
}

impl BitOrAssign for FsProperties {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// POSIX path configuration information (RFC 1813 3.3.20).
#[derive(Debug, Clone, PartialEq)]
pub struct PathConfig {
    pub file_attr: Option<FileAttr>,
    pub max_link: u32,
    pub max_name: u32,
    pub no_trunc: bool,
    pub chown_restricted: bool,
    pub case_insensitive: bool,
    pub case_preserving: bool,
}

/// Result returned by COMMIT (RFC 1813 3.3.21).
#[derive(Debug, Clone, PartialEq)]
pub struct CommitResult {
    pub file_attr: Option<FileAttr>,
    pub verifier: StableVerifier,
}

/// Virtual File System trait mirroring all NFSv3 procedures.
#[async_trait]
pub trait Vfs: Sync + Send {
    /// Procedure 0: NULL – sanity check / ping.
    async fn null(&self) -> VfsResult<()>;

    /// Procedure 1: GETATTR – fetch file attributes.
    async fn get_attr(&self, handle: &FileHandle) -> VfsResult<FileAttr>;

    /// Procedure 2: SETATTR – mutate file attributes with optional guard.
    async fn set_attr(
        &self,
        handle: &FileHandle,
        attr: SetAttr,
        guard: SetAttrGuard,
    ) -> VfsResult<WccData>;

    /// Procedure 3: LOOKUP – resolve a name within a directory.
    async fn lookup(&self, parent: &FileHandle, name: &FileName) -> VfsResult<LookupResult>;

    /// Procedure 4: ACCESS – evaluate requested access mask.
    async fn access(&self, handle: &FileHandle, mask: AccessMask) -> VfsResult<AccessResult>;

    /// Procedure 5: READLINK – read symbolic link contents.
    async fn read_link(&self, handle: &FileHandle) -> VfsResult<(SymlinkTarget, Option<FileAttr>)>;

    /// Procedure 6: READ – read file data.
    async fn read(&self, handle: &FileHandle, offset: u64, count: u32) -> VfsResult<ReadResult>;

    /// Procedure 7: WRITE – write file data with stability mode.
    async fn write(
        &self,
        handle: &FileHandle,
        offset: u64,
        data: &[u8],
        mode: WriteMode,
    ) -> VfsResult<WriteResult>;

    /// Procedure 8: CREATE – create and optionally initialize a regular file.
    async fn create(
        &self,
        parent: &FileHandle,
        name: &FileName,
        mode: CreateMode,
    ) -> VfsResult<CreatedNode>;

    /// Procedure 9: MKDIR – create a directory.
    async fn make_dir(
        &self,
        parent: &FileHandle,
        name: &FileName,
        attr: SetAttr,
    ) -> VfsResult<CreatedNode>;

    /// Procedure 10: SYMLINK – create a symbolic link.
    async fn make_symlink(
        &self,
        parent: &FileHandle,
        name: &FileName,
        target: &SymlinkTarget,
        attr: SetAttr,
    ) -> VfsResult<CreatedNode>;

    /// Procedure 11: MKNOD – create a special node.
    async fn make_node(
        &self,
        parent: &FileHandle,
        name: &FileName,
        node: SpecialNode,
    ) -> VfsResult<CreatedNode>;

    /// Procedure 12: REMOVE – delete a file.
    async fn remove(&self, parent: &FileHandle, name: &FileName) -> VfsResult<RemovalResult>;

    /// Procedure 13: RMDIR – delete an empty directory.
    async fn remove_dir(&self, parent: &FileHandle, name: &FileName) -> VfsResult<RemovalResult>;

    /// Procedure 14: RENAME – atomically move a file or directory.
    async fn rename(
        &self,
        from_parent: &FileHandle,
        from_name: &FileName,
        to_parent: &FileHandle,
        to_name: &FileName,
    ) -> VfsResult<RenameResult>;

    /// Procedure 15: LINK – create a hard link.
    async fn link(
        &self,
        source: &FileHandle,
        new_parent: &FileHandle,
        new_name: &FileName,
    ) -> VfsResult<LinkResult>;

    /// Procedure 16: READDIR – iterate directory entries.
    async fn read_dir(
        &self,
        handle: &FileHandle,
        cookie: DirectoryCookie,
        verifier: CookieVerifier,
        max_bytes: u32,
    ) -> VfsResult<ReadDirResult>;

    /// Procedure 17: READDIRPLUS – iterate directory entries with attributes.
    async fn read_dir_plus(
        &self,
        handle: &FileHandle,
        cookie: DirectoryCookie,
        verifier: CookieVerifier,
        max_bytes: u32,
        max_handles: u32,
    ) -> VfsResult<ReadDirPlusResult>;

    /// Procedure 18: FSSTAT – fetch dynamic filesystem statistics.
    async fn fs_stat(&self, handle: &FileHandle) -> VfsResult<FsStat>;

    /// Procedure 19: FSINFO – fetch static filesystem information.
    async fn fs_info(&self, handle: &FileHandle) -> VfsResult<FsInfo>;

    /// Procedure 20: PATHCONF – fetch POSIX path capabilities.
    async fn path_conf(&self, handle: &FileHandle) -> VfsResult<PathConfig>;

    /// Procedure 21: COMMIT – ensure previous writes reach stable storage.
    async fn commit(&self, handle: &FileHandle, offset: u64, count: u32)
        -> VfsResult<CommitResult>;
}

