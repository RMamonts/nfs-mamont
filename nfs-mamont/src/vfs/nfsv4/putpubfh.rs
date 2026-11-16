//! Defines the NFSv4 PUTPUBFH operation types, RFC 7530 §16.21.

/// PUTPUBFH arguments.
pub struct Args;

/// Success result (`PUTPUBFH4res`).
pub struct Success;

/// Fail result (`PUTPUBFH4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
