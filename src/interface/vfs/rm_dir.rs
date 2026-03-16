//! Defines NFSv3 [`RmDir`] interface.

use async_trait::async_trait;

use crate::interface::vfs;

/// Success result.
pub struct Success {
    /// Weak cache consistency data for the directory.
    pub wcc_data: vfs::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// Weak cache consistency data for the directory from [`Args::object`].
    pub dir_wcc: vfs::WccData,
}

/// [`RmDir::rm_dir`] arguments.
pub struct Args {
    /// A [`vfs::DirOpArgs`] structure identifying the directory entry
    /// to be removed.
    pub object: vfs::DirOpArgs,
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
    async fn rm_dir(&self, args: Args) -> Result<Success, Fail>;
}
