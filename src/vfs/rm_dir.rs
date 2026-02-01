//! Defines NFSv3 [`RmDir`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    /// Weak cache consistency data for the directory.
    pub wcc_data: vfs::WccData,
}

/// Fail result.
pub struct Fail {
    /// Weak cache consistency data for the directory, where.dir.
    /// TODO(use Args structure).
    pub dir_wcc: vfs::WccData,
}

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`RmDir::rm_dir`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`RmDir::rm_dir`] arguments.
pub struct Args {
    /// The file handle for the directory from which the subdirectory is to be removed.
    pub dir: file::Handle,
    /// The name of the subdirectory to be removed.
    pub name: String,
}

#[async_trait]
pub trait RmDir {
    /// Removes (deletes) a subdirectory from a directory.
    ///
    /// On some servers, the filename, ".", is illegal. These servers will return
    /// the error, [`vfs::Error::InvalidArgument`].
    ///
    /// On some servers, the filename, "..", is illegal. These servers will return
    /// the error, [`vfs::Error::Exist`].
    async fn rm_dir(&self, args: Args, promise: impl Promise);
}
