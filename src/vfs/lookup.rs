//! Defines NFSv3 [`Lookup`] interface.

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    object: file::Handle,
    object_attr: Option<file::Attr>,
    dir_attr: Option<file::Attr>,
}

/// Failed result.
pub struct Fail {
    error: vfs::Error,
    dir_attr: Option<file::Attr>,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Lookup::lookup`] result into.
pub trait Promise {
    fn keep(promise: Result);
}

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
