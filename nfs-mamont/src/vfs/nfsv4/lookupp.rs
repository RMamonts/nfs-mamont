//! Defines the NFSv4 LOOKUPP operation types, RFC 7530 §16.14.

/// LOOKUPP arguments.
pub struct Args;

/// Success result (`LOOKUPP4res`).
pub struct Success;

/// Fail result (`LOOKUPP4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
