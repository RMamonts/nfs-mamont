//! Defines the NFSv4 PUTFH operation types, RFC 7530 §16.20.

use super::file;

/// PUTFH arguments (`PUTFH4args`).
pub struct Args {
    /// Filehandle to replace the current filehandle with.
    pub object: file::Handle,
}

/// Success result (`PUTFH4res`).
pub struct Success;

/// Fail result (`PUTFH4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
