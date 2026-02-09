//! Defines NFSv3 [`Rename`] interface.

use async_trait::async_trait;

use crate::vfs::{self};

use super::file;

/// Success result.
pub struct Success {
    /// Weak cache consistency data for the directory, `from_dir`.
    pub from_dir_wcc: vfs::WccData,
    /// Weak cache consistency data for the directory, `to_dir`.
    pub to_dir_wcc: vfs::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// Weak cache consistency data for the directory, `from_dir`.
    pub from_dir_wcc: vfs::WccData,
    /// Weak cache consistency data for the directory, `to_dir`.
    pub to_dir_wcc: vfs::WccData,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Rename::rename`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`Rename::rename`] arguments.
pub struct Args {
    /// The file handle for the directory from which the entry is to be renamed.
    pub from_dir: file::Handle,
    /// The name of the entry that identifies the object to be renamed.
    pub from_name: String,
    /// The file handle for the directory to which the object is to be renamed.
    pub to_dir: file::Handle,
    /// The new name for the object.
    pub to_name: String,
}

#[async_trait]
pub trait Rename {
    /// Renames the file in the directory.
    ///
    /// The operation is required to be atomic to the client.
    ///
    /// [`Args::to_dir`] and [`Args::from_dir`] must reside on the same file system and server,
    /// means that the fsid fields in the attributes for the directories are the same. If they
    /// reside on different file systems, the error, [`vfs::Error::XDev`], is returned.
    ///
    /// Even though the operation is atomic, the status, [`vfs::Error::TooManyLinks`], may be
    /// returned if the server used a `"unlink/link/unlink"` sequence internally.
    ///
    /// A file handle may or may not become stale on a rename. However, server implementors are
    /// strongly encouraged to attempt to keep file handles from becoming stale in this fashion.
    ///
    /// On some servers, the filenames, "." and "..", are illegal as either from.name or to.name.
    /// In addition, neither [`Args::from_name`] nor [`Args::to_name`] can be an alias for
    /// [`Args::from_dir`]. These servers will return the error, [`vfs::Error::InvalidArgument`],
    /// in these cases.
    ///
    /// If the directory, [`Args::to_dir`], already contains an entry with the name,
    /// [`Args::to_name`], the source object must be compatible with the target: either both are
    /// non-directories or both are directories and the target must be empty. If compatible, the
    /// existing target is removed before the rename occurs. If they are not compatible or if the
    /// target is a directory but not empty, the server should return the error,
    /// [`vfs::Error::Exist`].
    ///
    /// If arguments pairs refer to the same file (they might be hard links of each other), then
    /// [`Rename::rename`] should perform no action and return [`Success`].
    async fn rename(&self, args: Args, promise: impl Promise);
}
