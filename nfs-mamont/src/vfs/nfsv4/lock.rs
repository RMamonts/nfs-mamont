//! Defines the NFSv4 LOCK operation types, RFC 7530 §16.10.

use std::io;

use crate::consts::nfsv4::NFS4_OPAQUE_LIMIT;

use super::file;

/// Type of lock (`nfs_lock_type4`), RFC 7530 §16.10.2.
pub enum LockType {
    /// READ_LT - a read lock.
    Read,
    /// WRITE_LT - a write lock.
    Write,
    /// READW_LT - a blocking read lock.
    ReadBlocking,
    /// WRITEW_LT - a blocking write lock.
    WriteBlocking,
}

pub struct LockId(String);

impl LockId {
    /// Creates a new [`LockId`] after validating its length.
    pub fn new(id: String) -> io::Result<Self> {
        if id.len() > NFS4_OPAQUE_LIMIT {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "name too long"));
        }
        if id.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "name is empty"));
        }
        Ok(LockId(id))
    }

    /// Consumes the wrapper and returns the inner [`String`].
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Returns the inner id as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The owner of file locking state (`lock_owner4`), RFC 7530 §2.2.14.
pub struct LockOwner {
    /// Client that owns the lock.
    pub clientid: file::ClientId,
    /// Opaque identifier of the lock owner.
    pub owner: LockId,
}

/// Locker describing the transition from an open owner to a new lock owner
/// (`open_to_lock_owner4`).
pub struct OpenToLockOwner {
    /// Sequence id of the open operation.
    pub open_seqid: file::SeqId,
    /// Open state id of the file.
    pub open_stateid: file::StateId,
    /// Sequence id of the lock operation.
    pub lock_seqid: file::SeqId,
    /// Owner of the new lock.
    pub lock_owner: LockOwner,
}

/// Locker describing an existing lock owner continuing to request locks
/// (`exist_lock_owner4`).
pub struct ExistLockOwner {
    /// State id of an existing lock.
    pub lock_stateid: file::StateId,
    /// Sequence id of the lock operation.
    pub lock_seqid: file::SeqId,
}

/// Union of ways to identify the lock owner (`locker4`), RFC 7530 §16.10.2.
pub enum Locker {
    /// Transition from an open owner to a new lock owner.
    NewOpen(OpenToLockOwner),
    /// An existing lock owner continues to request locks.
    Existing(ExistLockOwner),
}

/// LOCK arguments (`LOCK4args`).
pub struct Args {
    /// Type of lock to acquire.
    pub locktype: LockType,
    /// Whether the lock is being reclaimed in recovery.
    pub reclaim: bool,
    /// Byte offset of the start of the range to lock.
    pub offset: u64,
    /// Length of the range to lock.
    pub length: u64,
    /// Identity of the lock owner.
    pub locker: Locker,
}

/// Details of a lock that was denied (`LOCK4denied`), RFC 7530 §16.10.3.
pub struct Denied {
    /// Offset of the conflicting range.
    pub offset: u64,
    /// Length of the conflicting range.
    pub length: u64,
    /// Type of the conflicting lock.
    pub locktype: LockType,
    /// Owner of the conflicting lock.
    pub owner: LockOwner,
}

/// Success result (`LOCK4resok`).
pub struct Success {
    /// State id of the granted lock.
    pub lock_stateid: file::StateId,
}

/// Fail result (`LOCK4res`), RFC 7530 §16.10.3.
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
    /// Details of the conflicting lock, present when the request was
    /// denied (`NFS4ERR_DENIED`).
    pub denied: Option<Denied>,
}
