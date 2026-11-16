//! Defines the NFSv4 LOCKT operation types, RFC 7530 §16.11.

use super::{lock, Error};

/// LOCKT arguments (`LOCKT4args`).
pub struct Args {
    /// Type of lock to test.
    pub locktype: lock::LockType,
    /// Byte offset of the start of the range to test.
    pub offset: u64,
    /// Length of the range to test.
    pub length: u64,
    /// Owner of the prospective lock.
    pub owner: lock::LockOwner,
}

/// Success result (`LOCKT4res`).
pub struct Success;

/// Fail result (`LOCKT4res`), RFC 7530 §16.11.3.
pub struct Fail {
    /// Error on failure.
    pub error: Error,
    /// Details of the conflicting lock, present when the test found a
    /// conflicting lock (`NFS4ERR_DENIED`).
    pub denied: Option<lock::Denied>,
}
