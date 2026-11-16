//! Defines the NFSv4 NVERIFY operation types, RFC 7530 §16.15.

use super::file;

/// NVERIFY arguments (`NVERIFY4args`).
pub struct Args {
    /// Attributes that must not match the object's current attributes.
    pub obj_attributes: file::Attr,
}

/// Success result (`NVERIFY4res`).
pub struct Success;

/// Fail result (`NVERIFY4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
