//! Defines NFSv3 [`FsStat`] interface.

use async_trait::async_trait;

use super::file;

/// Success result.
pub struct Success {
    /// The attributes of the mount point.
    pub root_attr: Option<file::Attr>,
    /// The total size, in bytes, of the file system.
    pub total_bytes: u32,
    /// The amount of free space, in bytes, in the file system.
    pub free_bytes: u32,
    /// The amount of free space, in bytes, available to the
    /// user identified by the authentication information.
    /// (This reflects space that is reserved by the
    /// file system; it does not reflect any quota system
    /// implemented by the server.)
    pub available_bytes: u32,
    /// The total number of file slots in the file system. (On
    /// a UNIX server, this often corresponds to the number of
    /// inodes configured.)
    pub total_files: u32,
    /// The number of free file slots in the file system.
    pub free_files: u32,
    /// The number of free file slots that are available to the
    /// user corresponding to the authentication information.
    /// (This reflects slots that are reserved by the
    /// file system; it does not reflect any quota system
    /// implemented by the server.)
    pub available_files: u32,
    /// A measure of file system volatility.
    pub invarsec: u32,
}

/// Fail result.
pub struct Fail {
    /// The attributes of the mount point.
    pub root_attr: Option<file::Attr>,
}

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`ReadDir::read_dir`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`FsStat::fs_stat`] arguments.
#[derive(PartialEq)]
pub struct Args {
    /// A file handle identifying a mount point in the file system.
    pub root: file::Handle,
}

#[async_trait]
pub trait FsStat {
    /// Retrieves volatile file system state information.
    async fn fs_stat(&self, args: Args, promise: impl Promise);
}
