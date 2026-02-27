//! Defines NFSv3 [`MkDir`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    /// The file handle for the newly created directory.
    pub file: Option<file::Handle>,
    /// The attributes for the newly created subdirectory.
    pub attr: Option<file::Attr>,
    /// Weak cache consistency data for the [`Args::object`] dir.
    pub wcc_data: vfs::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// Weak cache consistency data for the directory, [`Args::object`] dir.
    pub dir_wcc: vfs::WccData,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`MkDir::mk_dir`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`MkDir::mk_dir`] arguments.
pub struct Args {
    /// The location of the subdirectory to be created.
    pub object: vfs::DirOpArgs,
    /// The initial attributes for the subdirectory.
    pub attr: super::set_attr::NewAttr,
}

#[async_trait]
pub trait MkDir {
    /// Creates a new subdirectory.
    ///
    /// Returns [`vfs::Error::Exist`] for "." or ".." `name`.
    async fn mk_dir(&self, args: Args, promise: impl Promise);
}
