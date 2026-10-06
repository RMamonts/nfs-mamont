//! Defines NLMv4 basic lock structures.
//!
//! Contains [`Nlm4Lock`] and [`OpaqueHandle`] types used by lock procedures.

use crate::vfs;

use super::{Name, OpaqueHandle};

/// This structure describes a lock request.
pub struct Nlm4Lock {
    /// Name of the client host making the lock request.
    pub caller_name: Name,
    /// Handle to the file to lock.
    pub file_handle: vfs::file::Handle,
    /// Host or process that is making the request.
    pub opaque_handle: OpaqueHandle,
    /// PID of the process making the request.
    pub system_identifier: i32,
    /// Offset for the lock region.
    pub lock_offset: u64,
    /// Length of the blocking region. An l_len of 0 means "to end of file".
    pub lock_length: u64,
}

impl Nlm4Lock {
    /// Creates a new lock request.
    ///
    /// # Parameters
    ///
    /// - `caller_name`: Name of the client host making the lock request.
    /// - `file_handle`: Handle to the file to lock.
    /// - `opaque_handle`: Host or process that is making the request.
    /// - `system_identifier`: PID of the process making the request.
    /// - `lock_offset`: Offset for the lock region.
    /// - `lock_length`: Length of the blocking region.
    ///
    /// # Returns
    ///
    /// Returns a new [`Nlm4Lock`] instance if the request is valid.
    pub fn new(
        caller_name: Name,
        file_handle: vfs::file::Handle,
        opaque_handle: OpaqueHandle,
        system_identifier: i32,
        lock_offset: u64,
        lock_length: u64,
    ) -> Self {
        Nlm4Lock {
            caller_name,
            file_handle,
            opaque_handle,
            system_identifier,
            lock_offset,
            lock_length,
        }
    }
}
