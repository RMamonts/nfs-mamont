//! Defines NFSv3 [`ReadLink`] interface.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    /// The data associated with the symbolic link.
    pub data: PathBuf,
    /// The post-operation attributes for the symbolic link.
    pub symlink_attr: Option<file::Attr>,
}

/// Fail result.
pub struct Fail {
    /// The post-operation attributes for the symbolic link.
    pub symlink_attr: Option<file::Attr>,
    /// Error on failure.
    pub error: vfs::Error,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`ReadLink::read_link`] result into.
#[async_trait]
pub trait Promise: Send {
    async fn keep(promise: Result);
}

/// [`ReadLink::read_link`] arguments.
pub struct Args {
    /// The file handle for a symbolic link (file system object of type [`file::Type::Symlink`]).
    pub file: file::Handle,
}

#[async_trait]
pub trait ReadLink {
    /// Reads the data associated with a symbolic link.
    ///
    /// The [`ReadLink::read_link`] operation is only allowed on
    /// objects of type [`file::Type::Symlink`]. The server should return the error,
    /// [`vfs::Error::InvalidArgument`], if the object is not of type, [`file::Type::Symlink`].
    async fn read_link(&self, args: Args, promise: impl Promise);
}
