//! Defines NFSv3 [`Symlink`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    /// The file handle for the newly created symbolic link.
    pub file: Option<file::Handle>,
    /// The attributes for the newly created symbolic link.
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

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Symlink::symlink`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`Symlink::symlink`] arguments.
pub struct Args {
    /// The file handle for the directory in which the symbolic link to be created.
    pub dir: file::Handle,
    /// The name that is to be associated with the created symbolic link.
    pub name: file::FileName,
    /// The initial attributes for the symbolic link.
    pub attr: super::set_attr::NewAttr,
    /// The symbolic link data.
    pub path: file::FilePath,
}

#[async_trait]
pub trait Symlink {
    /// Creates a new symbolic link.
    ///
    /// Returns [`vfs::Error::Exist`] for "." or ".." `name`.
    ///
    /// For symbolic links, the actual file system node and its contents are expected to be
    /// created in a single atomic operation. That is, once the symbolic link is visible,
    /// there must not be a window where a [`super::read_link::ReadLink::read_link`] would fail or
    /// return incorrect data.
    async fn symlink(&self, args: Args, promise: impl Promise);
}
