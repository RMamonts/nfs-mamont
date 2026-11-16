//! Defines the NFSv4 LOCKU operation types, RFC 7530 §16.12.

use super::{file, lock};

/// LOCKU arguments (`LOCKU4args`).
pub struct Args {
    /// Type of lock to unlock.
    pub locktype: lock::LockType,
    /// Sequence id providing for the correct ordering of the operation.
    pub seqid: file::SeqId,
    /// State id of the lock being unlocked.
    pub lock_stateid: file::StateId,
    /// Byte offset of the start of the range to unlock.
    pub offset: u64,
    /// Length of the range to unlock.
    pub length: u64,
}

/// Success result (`LOCKU4res`).
pub struct Success {
    /// State id of the lock after the unlock.
    pub lock_stateid: file::StateId,
}

/// Fail result (`LOCKU4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
