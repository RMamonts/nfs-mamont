//! Defines the NFSv4 GETATTR operation types, RFC 7530 §16.7.

use super::file::{Attr, Bitmap};

/// GETATTR arguments (`GETATTR4args`).
pub struct Args {
    /// Bitmask of attributes the client is requesting.
    pub attr_request: Bitmap,
}

/// Success result (`GETATTR4resok`).
pub struct Success {
    /// Attributes of the object.
    pub obj_attributes: Attr,
}

/// Fail result (`GETATTR4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
