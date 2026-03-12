//! Defines NFSv3 [`GetAttr`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

/// Success result.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct Success {
    pub object: file::Attr,
}

/// Fail result.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
}

/// [`GetAttr::get_attr`] arguments.
#[derive(Debug)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, PartialEq, Clone))]
pub struct Args {
    /// File handle of an object whose attributes are to be retrieved.
    pub file: file::Handle,
}

#[async_trait]
pub trait GetAttr {
    /// Retrieves the attributes for a specified file system object.
    async fn get_attr(&self, args: Args) -> Result<Success, Fail>;
}
