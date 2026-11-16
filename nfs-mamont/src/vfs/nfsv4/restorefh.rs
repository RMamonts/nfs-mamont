//! Defines the NFSv4 RESTOREFH operation types, RFC 7530 §16.29.

/// RESTOREFH arguments.
pub struct Args;

/// Success result (`RESTOREFH4res`).
pub struct Success;

/// Fail result (`RESTOREFH4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
