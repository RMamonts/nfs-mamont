//! Defines the NFSv4 ACCESS operation types, RFC 7530 §16.1.
use crate::vfs::nfs_shared::access::Mask;

/// ACCESS arguments (`ACCESS4args`).
pub struct Args {
    /// Bitmask of access permissions to check.
    pub access: Mask,
}

/// Success result (`ACCESS4resok`).
pub struct Success {
    /// Bitmask of access rights for which the server can verify reliably.
    pub supported: Mask,
    /// Bitmask of access rights available to the user for the given file
    /// handle.
    pub access: Mask,
}

/// Fail result (`ACCESS4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
