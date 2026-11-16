//! Defines the NFSv4 OPENATTR operation types, RFC 7530 §16.17.

/// OPENATTR arguments (`OPENATTR4args`).
pub struct Args {
    /// Whether to create the named attribute directory if it does not exist.
    pub createdir: bool,
}

/// Success result (`OPENATTR4res`).
pub struct Success;

/// Fail result (`OPENATTR4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
