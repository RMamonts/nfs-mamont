//! Defines NFSv3 [`ReadLink`] interface.

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    /// The post-operation attributes for the symbolic link.
    pub symlink_attr: Option<file::Attr>,
    /// The data associated with the symbolic link.
    pub data: file::Path,
}

/// Fail result.
pub struct Fail {
    /// The post-operation attributes for the symbolic link.
    pub symlink_attr: Option<file::Attr>,
    /// Error on failure.
    pub error: vfs::Error,
}

/// [`ReadLink::read_link`] arguments.
pub struct Args {
    /// The file handle for a symbolic link (file system object of type [`file::Type::Symlink`]).
    pub file: file::Handle,
}

pub trait ReadLink {
    /// Reads the data associated with a symbolic link.
    ///
    /// The [`ReadLink::read_link`] operation is only allowed on
    /// objects of type [`file::Type::Symlink`]. The server should return the error,
    /// [`vfs::Error::InvalidArgument`], if the object is not of type, [`file::Type::Symlink`].
    fn read_link(
        &self,
        args: Args,
    ) -> impl std::future::Future<Output = Result<Success, Fail>> + Send;
}
