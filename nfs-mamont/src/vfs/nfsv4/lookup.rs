//! Defines the NFSv4 LOOKUP operation types, RFC 7530 §16.13.

use crate::vfs::nfsv4::file;

/// LOOKUP arguments (`LOOKUP4args`).
pub struct Args {
    /// Name of the object to find in the directory specified by the current
    /// filehandle.
    pub objname: file::Name,
}

/// Success result (`LOOKUP4res`).
pub struct Success;

/// Fail result (`LOOKUP4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
