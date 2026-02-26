//! Defines NFSv3 Virtual File System interface --- [`Vfs`].

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
    /// operation. Two examples are attempting a [`read_link`] on an
    /// object other than a symbolic link or attempting to
    /// [`set_attr`] a time field on a server that does not support
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
    /// [`set_attr`] operation.
    NotSync,
    /// [`read_dir`] or [`read_dir_plus`] cookie is stale.
    BadCookie,
    /// Operation is not supported.
    NotSupported,
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
    JUKEBOX,
}

/// TODO(i.erin)
#[derive(Clone)]
pub struct WccData {
    pub before: Option<file::WccAttr>,
    pub after: Option<file::Attr>,
}

/// This struct represents diropargs3 from NFS3
///
/// Can be foud in NFS3 RFC 1813, <https://datatracker.ietf.org/doc/html/rfc1813#autoid-15>
pub struct DirOpArgs {
    /// The file handle for the directory from which the subdirectory is to be removed.
    pub dir: file::Handle,
    /// The name of the subdirectory to be removed.
    pub name: String,
}

pub trait Vfs {}
