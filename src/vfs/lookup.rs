//! Defines NFSv3 [`Lookup`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    pub file: file::Handle,
    pub file_attr: Option<file::Attr>,
    pub dir_attr: Option<file::Attr>,
}

/// Failed result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// The post-operation attributes of the directory
    pub dir_attr: Option<file::Attr>,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Lookup::lookup`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`Lookup::lookup`] arguments.
pub struct Args {
    /// File handle for the directory to search.
    pub parent: file::Handle,
    /// File name to be searched for.
    pub name: String,
}

#[async_trait]
pub trait Lookup {
    /// Searches a directory for a specific name and returns the file handle for the corresponding
    /// file system object.
    ///
    /// Note that this procedure does not follow symbolic links.
    async fn lookup(&self, args: Args, promise: impl Promise);
}
