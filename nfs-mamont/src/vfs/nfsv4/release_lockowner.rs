//! Defines the NFSv4 RELEASE_LOCKOWNER operation types, RFC 7530 §16.37.

use crate::vfs::nfsv4::lock;

/// RELEASE_LOCKOWNER arguments (`RELEASE_LOCKOWNER4args`).
pub struct Args {
    /// Lock owner whose state is to be released.
    pub lock_owner: lock::LockOwner,
}

/// Success result (`RELEASE_LOCKOWNER4res`).
pub struct Success;

/// Fail result (`RELEASE_LOCKOWNER4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
