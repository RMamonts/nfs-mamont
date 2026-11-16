//! Defines the NFSv4 GETFH operation types, RFC 7530 §16.8.

use super::file::Handle;

/// GETFH arguments.
pub struct Args;

/// Success result (`GETFH4resok`).
pub struct Success {
    pub object: Handle,
}

/// Fail result (`GETFH4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
