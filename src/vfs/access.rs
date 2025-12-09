//! Defines NFSv3 [`Lookup`] interface.

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    object_attr: Option<file::Attr>,
    access: Mask,
}

/// Fail result.
pub struct Fail {
    object_attr: Option<file::Attr>,
    error: vfs::Error,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Lookup::lookup`] result into.
pub trait Promise {
    fn keep(promise: Result);
}

/// Mask of [`Access::access`] rights.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Mask(u32);

impl Mask {
    const READ: u32 = 0x001;
    const LOOKUP: u32 = 0x002;
    const MODIFY: u32 = 0x004;
    const EXTEND: u32 = 0x008;
    const DELETE: u32 = 0x010;
    const EXECUTE: u32 = 0x0020;

    pub fn new(mask: u32) -> Option<Self> {
        let possible =
            Self::READ | Self::LOOKUP | Self::MODIFY | Self::EXECUTE | Self::DELETE | Self::EXECUTE;

        if mask & (!possible) != 0 {
            return None;
        }

        Some(Self(mask))
    }

    pub fn is_read(&self) -> bool {
        self.0 & Self::READ != 0
    }

    pub fn is_lookup(&self) -> bool {
        self.0 & Self::LOOKUP != 0
    }

    pub fn is_modify(&self) -> bool {
        self.0 & Self::MODIFY != 0
    }

    pub fn is_extend(&self) -> bool {
        self.0 & Self::EXTEND != 0
    }

    pub fn is_delete(&self) -> bool {
        self.0 & Self::DELETE != 0
    }

    pub fn is_execute(&self) -> bool {
        self.0 & Self::EXECUTE != 0
    }
}

pub trait Access {
    /// Determines the access rights that a user, as identified by the credentials
    /// in the request, has with respect to a file system object.
    ///
    /// # Parameters:
    ///
    /// * `file` --- file handle for the file system object to which access is to be checked.
    /// * `access` --- a bit mask of access permissions to check.
    ///
    ///
    async fn access(&self, file: file::Handle, mask: Mask, promise: impl Promise);
}
