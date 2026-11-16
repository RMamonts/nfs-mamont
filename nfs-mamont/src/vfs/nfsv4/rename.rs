//! Defines the NFSv4 RENAME operation types, RFC 7530 §16.27.

use crate::vfs::nfsv4::file::{ChangeInfo, Name};

/// RENAME arguments (`RENAME4args`).
pub struct Args {
    /// Name of the object to rename in the source directory (saved filehandle).
    pub old_name: Name,
    /// New name of the object in the target directory (current filehandle).
    pub new_name: Name,
}

/// Success result (`RENAME4resok`).
pub struct Success {
    /// Change information for the source directory.
    pub source_cinfo: ChangeInfo,
    /// Change information for the target directory.
    pub target_cinfo: ChangeInfo,
}

/// Fail result (`RENAME4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
