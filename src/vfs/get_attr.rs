//! Defines NFSv3 [`GetAttr`] interface.

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

pub trait GetAttr {
    /// Retrieves the attributes for a specified file system object.
    fn get_attr(
        &self,
        args: Args,
    ) -> impl std::future::Future<Output = Result<Success, Fail>> + Send;
}
