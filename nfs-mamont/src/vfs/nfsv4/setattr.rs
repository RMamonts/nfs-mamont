//! Defines the NFSv4 SETATTR operation types, RFC 7530 §16.32.

use crate::vfs::nfsv4::file;

/// SETATTR arguments (`SETATTR4args`).
pub struct Args {
    /// State id providing the byte-range locking context for the operation.
    pub stateid: file::StateId,
    /// The attributes to set on the object.
    pub obj_attributes: file::Attr,
}

/// Success result (`SETATTR4res`).
pub struct Success {
    /// Bitmask of the attributes that were successfully set.
    pub attrsset: file::Bitmap,
}

/// Fail result (`SETATTR4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
