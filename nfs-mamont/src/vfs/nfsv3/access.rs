//! Defines NFSv3 [`Access`] interface.

use crate::rpc::auth::Credential;
use crate::vfs::nfs_shared::access::Mask;

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

/// [`Access::access`] arguments.
pub struct Args {
    /// File handle for the file system object to which access is to be checked
    pub file: file::Handle,
    /// Mask of access permissions to check
    pub mask: Mask,
}

#[trait_variant::make(Send)]
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
    async fn access(&self, args: Args, cred: &Credential) -> Result<Success, Fail>;
}
