//! Defines NFSv3 [`FsStat`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    /// The attributes of the mount point.
    pub root_attr: Option<file::Attr>,
    /// The total size, in bytes, of the file system.
    pub total_bytes: u64,
    /// The amount of free space, in bytes, in the file system.
    pub free_bytes: u64,
    /// The amount of free space, in bytes, available to the
    /// user identified by the authentication information.
    /// (This reflects space that is reserved by the
    /// file system; it does not reflect any quota system
    /// implemented by the server.)
    pub available_bytes: u64,
    /// The total number of file slots in the file system. (On
    /// a UNIX server, this often corresponds to the number of
    /// inodes configured.)
    pub total_files: u64,
    /// The number of free file slots in the file system.
    pub free_files: u64,
    /// The number of free file slots that are available to the
    /// user corresponding to the authentication information.
    /// (This reflects slots that are reserved by the
    /// file system; it does not reflect any quota system
    /// implemented by the server.)
    pub available_files: u64,
    /// A measure of file system volatility.
    pub invarsec: u32,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// The attributes of the mount point.
    pub root_attr: Option<file::Attr>,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`ReadDir::read_dir`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

#[async_trait]
pub trait FsStat {
    /// Retrieves volatile file system state information.
    ///
    /// # Parameters:
    ///
    /// * `root` --- A file handle identifying a mount point in the file system.
    async fn fs_stat(&self, root: file::Handle, promise: impl Promise);
}
