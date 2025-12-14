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
    pub dir_attr: Option<file::Attr>,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Lookup::lookup`] result into.
#[async_trait]
pub trait Promise {
    fn keep(promise: Result);
}

#[async_trait]
pub trait Lookup {
    /// Searches a directory for a specific name and returns the file handle for the corresponding
    /// file system object.
    ///
    /// # Parameters:
    ///
    /// * `parent` --- the file handle for the directory to search.
    /// * `name` --- the file name to be searched for.
    /// * `proimise` --- TODO.
    ///
    /// Note that this procedure does not follow symbolic links.
    async fn lookup(&self, parent: file::Handle, name: String, promise: impl Promise);
}
