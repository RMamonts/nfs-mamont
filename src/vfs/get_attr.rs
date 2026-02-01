//! Defines NFSv3 [`GetAttr`] interface.

use async_trait::async_trait;

use super::file;
use crate::vfs;

pub type Result = std::result::Result<Success, ()>;

pub struct Success {
    pub object: file::Handle,
}

/// Defines callback to pass [`GetAttr::get_attr`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(attr: vfs::Result<file::Attr>);
}

/// [`GetAttr::get_attr`] aguments.
pub struct Args {
    /// File handle of an object whose attributes are to be retrieved.
    pub file: file::Handle,
}

#[async_trait]
pub trait GetAttr {
    /// Retrieves the attributes for a specified file system object.
    async fn get_attr(&self, args: Args, promise: impl Promise);
}
