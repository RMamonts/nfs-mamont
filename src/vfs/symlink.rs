//! Defines NFSv3 [`Symlink`] interface.

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
    /// Weak cache consistency data for the directory from [`Args::object`].
    pub dir_wcc: vfs::WccData,
}

/// [`Symlink::symlink`] arguments.
pub struct Args {
    /// The location of the symbolic link to be created.
    pub object: vfs::DirOpArgs,
    /// The initial attributes for the symbolic link.
    pub attr: super::set_attr::NewAttr,
    /// The symbolic link data.
    pub path: file::Path,
}

pub trait Symlink {
    /// Creates a new symbolic link.
    ///
    /// Returns [`vfs::Error::Exist`] for "." or ".." `name`.
    ///
    /// For symbolic links, the actual file system node and its contents are expected to be
    /// created in a single atomic operation. That is, once the symbolic link is visible,
    /// there must not be a window where a [`super::read_link::ReadLink::read_link`] would fail or
    /// return incorrect data.
    fn symlink(
        &self,
        args: Args,
    ) -> impl std::future::Future<Output = Result<Success, Fail>> + Send;
}
