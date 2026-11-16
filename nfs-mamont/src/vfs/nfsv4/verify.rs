//! Defines the NFSv4 VERIFY operation types, RFC 7530 §16.35.

use crate::vfs::nfsv4::file;

/// VERIFY arguments (`VERIFY4args`).
pub struct Args {
    /// Attributes to verify against the current filehandle object.
    pub obj_attributes: file::Attr,
}

/// Success result (`VERIFY4res`).
pub struct Success;

/// Fail result (`VERIFY4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
