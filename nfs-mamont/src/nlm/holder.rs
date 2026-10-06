//! Defines NLMv4 lock holder structure.
//!
//! Contains [`Nlm4Holder`] which represents the current holder of a lock.

use super::OpaqueHandle;

/// This structure indicates the holder of a lock.
pub struct Nlm4Holder {
    /// Tells whether the holder has an exclusive lock or a shared lock.
    pub exclusive: bool,
    /// PID of the process holding the lock.
    pub system_identifier: i32,
    /// Host or process that is holding the lock.
    pub opaque_handle: OpaqueHandle,
    /// Offset for the lock region.
    pub lock_offset: u64,
    /// Length of the blocking region. An l_len of 0 means "to end of file".
    pub lock_length: u64,
}
