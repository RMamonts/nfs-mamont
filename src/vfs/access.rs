//! Defines NFSv3 [`Access`] interface.

use async_trait::async_trait;

use super::{file, Error};

/// Success result.
pub struct Success {
    pub object_attr: Option<file::Attr>,
    pub access: Mask,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: Error,
    pub object_attr: Option<file::Attr>,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Access::access`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

// TODO: implement Mask, issue #27
/// Mask of [`Access::access`] rights.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Mask(pub u32);

/// [`Access::access`] arguments.
pub struct Args {
    /// File handle for the file system object to which access is to be checked
    pub file: file::Handle,
    /// Mask of access permissions to check
    pub mask: Mask,
}

#[async_trait]
pub trait Access {
    /// Determines the access rights that a user, as identified by the credentials
    /// in the request, has with respect to a file system object.
    ///
    /// The results of this procedure are necessarily advisory in
    /// nature.  That is, a return status of [`Ok`] and the
    /// appropriate bit set in the bit mask does not imply that
    /// such access will be allowed to the file system object in
    /// the future, as access rights can be revoked by the server
    /// at any time.
    async fn access(&self, args: Args, promise: impl Promise);
}
