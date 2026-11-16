//! Defines the NFSv4 READLINK operation types, RFC 7530 §16.25.

/// READLINK arguments.
pub struct Args;

/// Success result (`READLINK4resok`).
pub struct Success {
    /// Link data of the symbolic link, a UTF-8 string opaque to the server.
    /// TODO: find out, how field is used and define type
    pub link: Vec<u8>,
}

/// Fail result (`READLINK4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
