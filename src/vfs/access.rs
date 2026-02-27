//! Defines NFSv3 [`Access`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    pub object_attr: Option<file::Attr>,
    pub access: Mask,
}

/// Fail result.
pub struct Fail {
    pub object_attr: Option<file::Attr>,
    pub error: vfs::Error,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Access::access`] result into.
#[async_trait]
pub trait Promise: Send {
    async fn keep(promise: Result);
}

/// Mask of [`Access::access`] rights.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Mask(u32);

impl Mask {
    pub const READ: u32 = 0x0001;
    pub const LOOKUP: u32 = 0x0002;
    pub const MODIFY: u32 = 0x0004;
    pub const EXTEND: u32 = 0x0008;
    pub const DELETE: u32 = 0x0010;
    pub const EXECUTE: u32 = 0x0020;

    pub const ALL: u32 =
        Self::READ | Self::LOOKUP | Self::MODIFY | Self::EXTEND | Self::DELETE | Self::EXECUTE;

    pub fn from_wire(raw: u32) -> Self {
        Self(raw & Self::ALL)
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn contains(self, flag: u32) -> bool {
        self.0 & flag == flag
    }
}

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
