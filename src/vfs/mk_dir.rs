//! Defines NFSv3 [`MkDir`] interface.

use async_trait::async_trait;

use crate::vfs::{self};

use super::file;

/// Success result.
pub struct Success {
    /// The file handle for the newly created directory.
    pub file: Option<file::Handle>,
    /// The attributes for the newly created subdirectory.
    pub attr: Option<file::Attr>,
    /// Weak cache consistency data for the directory.
    pub wcc_data: vfs::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// Weak cache consistency data for the directory, where.dir.
    /// TODO(use Args structure).
    pub dir_wcc: vfs::WccData,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`MkDir::mk_dir`] result into.
#[async_trait]
pub trait Promise {
    fn keep(promise: Result);
}

#[async_trait]
pub trait MkDir {
    /// Creates a new subdirectory.
    ///
    /// # Parameters:
    ///
    /// * `dir` --- The file handle for the directory in which the subdirectory is to be created.
    /// * `name` --- The name that is to be associated with the created subdirectory.
    /// * `how` --- The initial attributes for the subdirectory.
    ///
    /// Returns [`vfs::Error::Exist`] for "." or ".." `name`.
    async fn mk_dir(
        &self,
        dir: file::Handle,
        name: String,
        attr: super::set_attr::NewAttr,
        promise: impl Promise,
    );
}
