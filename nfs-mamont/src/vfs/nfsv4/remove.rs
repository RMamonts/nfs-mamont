//! Defines the NFSv4 REMOVE operation types, RFC 7530 §16.26.

use crate::vfs::nfsv4::file::{ChangeInfo, Name};

/// REMOVE arguments (`REMOVE4args`).
pub struct Args {
    /// Name of the directory entry to remove.
    pub target: Name,
}

/// Success result (`REMOVE4resok`).
pub struct Success {
    /// Change information for the directory from which the entry was removed.
    pub cinfo: ChangeInfo,
}

/// Fail result (`REMOVE4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
