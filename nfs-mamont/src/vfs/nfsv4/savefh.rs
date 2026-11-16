//! Defines the NFSv4 SAVEFH operation types, RFC 7530 §16.30.

/// SAVEFH arguments.
pub struct Args;

/// Success result (`SAVEFH4res`).
pub struct Success;

/// Fail result (`SAVEFH4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
