//! Defines NFSv3 Virtual File System interface --- [`Vfs`].

use async_trait::async_trait;
use num_derive::{FromPrimitive, ToPrimitive};

pub mod access;
pub mod commit;
pub mod create;
pub mod file;
pub mod fs_info;
pub mod fs_stat;
pub mod get_attr;
pub mod link;
pub mod lookup;
pub mod mk_dir;
pub mod mk_node;
pub mod path_conf;
pub mod read;
pub mod read_dir;
pub mod read_dir_plus;
pub mod read_link;
pub mod remove;
pub mod rename;
pub mod rm_dir;
pub mod set_attr;
pub mod symlink;
pub mod write;

/// Result of [`Vfs`] operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Maximum length of name passed into [`Vfs`] methods.
pub const MAX_NAME_LEN: usize = 255;

/// Maximum length of file path passed into [`Vfs`] methods.
pub const MAX_PATH_LEN: usize = 1024;

/// Represents `OK` variant in enum `nfsstat3`, that indicates of successful operation
pub const STATUS_OK: usize = 0;

/// [`Vfs`] errors.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ToPrimitive, FromPrimitive)]
pub enum Error {
    /// Not owner. The operation was not allowed because the
    /// caller is either not a privileged user (root) or not the
    /// owner of the target of the operation.
    Permission = 1,
    // Assume NOENT.
    /// No such file or directory. The file or directory name
    /// specified does not exist.
    NoEntry = 2,
    /// I/O error. A hard error (for example, a disk error)
    /// occurred while processing the requested operation.
    IO = 5,
    /// I/O error. No such device or address.
    NXIO = 6,
    /// Permission denied. The caller does not have the correct
    /// permission to perform the requested operation. Contrast
    /// this with NFS3ERR_PERM, which restricts itself to owner
    /// or privileged user permission failures.
    Access = 13,
    /// File exists. The file specified already exists.
    Exist = 17,
    /// Attempt to do a cross-device hard link.
    XDev = 18,
    /// No such device.
    NoDev = 19,
    /// Not a directory. The caller specified a non-directory in
    /// a directory operation.
    NotDir = 20,
    /// Is a directory. The caller specified a directory in a
    /// non-directory operation.
    IsDir = 21,
    /// Invalid argument or unsupported argument for an
    /// operation. Two examples are attempting a [`read_link`] on an
    /// object other than a symbolic link or attempting to
    /// [`set_attr`] a time field on a server that does not support
    /// this operation.
    InvalidArgument = 22,
    /// File too large. The operation would have caused a file to
    /// grow beyond the server's limit.
    FileTooLarge = 27,
    /// No space left on device. The operation would have caused
    /// the server's file system to exceed its limit.
    NoSpace = 28,
    /// Read-only file system. A modifying operation was
    /// attempted on a read-only file system.
    ReadOnlyFs = 30,
    /// Too many hard links.
    TooManyLinks = 31,
    /// The filename in an operation was too long.
    NameTooLong = 63,
    /// An attempt was made to remove a directory that was not
    /// empty.
    NotEmpty = 66,
    /// Resource (quota) hard limit exceeded. The user's resource
    /// limit on the server has been exceeded.
    QuotaExceeded = 69,
    /// Invalid file handle. The file handle given in the
    /// arguments was invalid. The file referred to by that file
    /// handle no longer exists or access to it has been
    /// revoked.
    StaleFile = 70,
    /// Too many levels of remote in path. The file handle given
    /// in the arguments referred to a file on a non-local file
    /// system on the server.
    TooManyLevelsOfRemote = 71,
    /// Illegal NFS file handle. The file handle failed internal
    /// consistency checks.
    BadFileHandle = 10001,
    /// Update synchronization mismatch was detected during a
    /// [`set_attr`] operation.
    NotSync = 10002,
    /// [`read_dir`] or [`read_dir_plus`] cookie is stale.
    BadCookie = 10003,
    /// Operation is not supported.
    NotSupported = 10004,
    /// Buffer or request is too small.
    TooSmall = 10005,
    /// An error occurred on the server which does not map to any
    /// of the legal NFS version 3 protocol error values.  The
    /// client should translate this into an appropriate error.
    /// UNIX clients may choose to translate this to EIO.
    ServerFault = 10006,
    /// An attempt was made to create an object of a type not
    /// supported by the [`crate::vfs`] implementation.
    BadType = 10007,
    /// The server initiated the request, but was not able to
    /// complete it in a timely fashion. The client should wait
    /// and then try the request with a new RPC transaction ID.
    /// to process a file that has been migrated. In this case,
    /// the server should start the immigration process and
    /// respond to client with this error.
    JUKEBOX = 10008,
}

#[derive(Clone)]
pub struct WccData {
    pub before: Option<file::WccAttr>,
    pub after: Option<file::Attr>,
}

/// This struct represents the generic `diropargs3` structure from NFSv3.
///
/// It is used by several directory operations (for example, create, mkdir, rmdir,
/// remove, symlink, mknod, link, and rename). See NFSv3 RFC 1813:
/// <https://datatracker.ietf.org/doc/html/rfc1813#autoid-15>
pub struct DirOpArgs {
    /// The file handle for the directory.
    pub dir: file::Handle,
    /// The name of the entry within the directory.
    pub name: file::Name,
}

#[async_trait]
pub trait RootHandle {
    /// Returns the file handle for the exported root directory.
    async fn root_handle(&self) -> file::Handle;
}

pub trait Vfs:
    RootHandle
    + get_attr::GetAttr
    + set_attr::SetAttr
    + lookup::Lookup
    + access::Access
    + read_link::ReadLink
    + read::Read
    + write::Write
    + create::Create
    + mk_dir::MkDir
    + symlink::Symlink
    + mk_node::MkNode
    + remove::Remove
    + rm_dir::RmDir
    + rename::Rename
    + link::Link
    + read_dir::ReadDir
    + read_dir_plus::ReadDirPlus
    + fs_stat::FsStat
    + fs_info::FsInfo
    + path_conf::PathConf
    + commit::Commit
{
}
