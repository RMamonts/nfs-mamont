//! Defines NFSv3 [`GetAttr`] interface.

use async_trait::async_trait;
use std::path::Path;

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    pub object: file::Attr,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
}

/// [`GetAttr::get_attr`] aguments.
pub struct Args {
    /// File handle of an object whose attributes are to be retrieved.
    pub file: file::Handle,
}

#[async_trait]
pub trait GetAttr {
    /// Retrieves the attributes for a specified file system object.
    async fn get_attr(&self, path: &Path) -> Result<Success, Fail>;
}
