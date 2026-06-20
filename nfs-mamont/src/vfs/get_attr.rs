//! Defines NFSv3 [`GetAttr`] interface.

use nfs_mamont_derive::XDRSize;

use crate::vfs;

use super::file;

/// Success result.
#[derive(XDRSize)]
pub struct Success {
    pub object: file::Attr,
}

/// Fail result.
#[derive(XDRSize)]
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
}

/// [`GetAttr::get_attr`] aguments.
pub struct Args {
    /// File handle of an object whose attributes are to be retrieved.
    pub file: file::Handle,
}

#[trait_variant::make(Send)]
pub trait GetAttr {
    /// Retrieves the attributes for a specified file system object.
    async fn get_attr(&self, args: Args) -> Result<Success, Fail>;
}
