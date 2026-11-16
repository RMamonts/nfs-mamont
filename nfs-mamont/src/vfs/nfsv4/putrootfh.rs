//! Defines the NFSv4 PUTROOTFH operation types, RFC 7530 §16.22.

/// PUTROOTFH arguments.
pub struct Args;

/// Success result (`PUTROOTFH4res`).
pub struct Success;

/// Fail result (`PUTROOTFH4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
