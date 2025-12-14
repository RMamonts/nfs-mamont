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
pub trait Promise {
    fn keep(promise: Result);
}

#[async_trait]
pub trait Rename {
    /// Renames the file in the directory.
    ///
    /// # Parameters:
    ///
    /// * `from_dir` --- The file handle for the directory from which the entry is to be renamed.
    /// * `from_name` --- The name of the entry that identifies the object to be renamed.
    /// * `to_dir` --- The file handle for the directory to which the object is to be renamed.
    /// * `to_name` --- The new name for the object.
    ///
    /// The operation is required to be atomic to the client.
    ///
    /// `to_dir` and `from_dir` must reside on the same file system and server, means
    /// that the fsid fields in the attributes for the directories are the same. If they reside on different file systems,
    /// the error, [`vfs::Error::XDev`], is returned.
    ///
    /// Even though the operation is atomic, the status, [`vfs::Error::TooManyLinks`], may be
    /// returned if the server used a "unlink/link/unlink" sequence internally.
    ///
    /// A file handle may or may not become stale on a rename. However, server implementors are
    /// strongly encouraged to attempt to keep file handles from becoming stale in this fashion.
    ///
    /// On some servers, the filenames, "." and "..", are illegal as either from.name or to.name.
    /// In addition, neither from.name nor to.name can be an alias for from.dir. These servers will
    /// return the error, [`vfs::Error::InvalidArgument`], in these cases.
    ///
    /// If the directory, `to_dir`, already contains an entry with
    /// the name, `to_name`, the source object must be compatible
    /// with the target: either both are non-directories or both
    /// are directories and the target must be empty. If
    /// compatible, the existing target is removed before the
    /// rename occurs. If they are not compatible or if the target
    /// is a directory but not empty, the server should return the
    /// error, [`vfs::Error::Exist`].
    ///
    /// If arguments pairs refer to the same file (they might be hard links of each other), then
    /// [`Rename::rename`] should perform no action and return [`Success`].
    async fn rename(
        &self,
        from_dir: file::Handle,
        from_name: String,
        to_dir: file::Handle,
        to_name: String,
        promise: impl Promise,
    );
}
