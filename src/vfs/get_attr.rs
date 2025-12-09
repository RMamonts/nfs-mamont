//! Defines NFSv3 [`GetAttr`] interface.

use crate::vfs;

use super::file;

/// Defines callback to pass [`GetAttr::get_attr`] result into.
pub trait Promise {
    async fn keep(attr: vfs::Result<file::Attr>);
}

pub trait GetAttr {
    /// Retrieves the attributes for a specified file system object.
    ///
    /// # Parameters:
    ///
    /// * `file` --- file handle of an object whose attributes are to be retrieved.
    /// * `promise` --- promise to perform the required operation and return the result.
    async fn get_attr(&self, file: file::Handle, promise: impl Promise);
}
