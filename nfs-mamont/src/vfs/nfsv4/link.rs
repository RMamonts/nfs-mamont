//! Defines the NFSv4 LINK operation types, RFC 7530 §16.11.

use super::file::{ChangeInfo, Name};

/// LINK arguments.
pub struct Args {
    pub name: Name,
}

/// Success result (`LINK4resok`).
pub struct Success {
    pub info: ChangeInfo,
}

/// Fail result (`LINK4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
