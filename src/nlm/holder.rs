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
    /// Length of the blocking region. A l_len of 0 means "to end of file".
    pub lock_length: u64,
}

impl Nlm4Holder {
    /// Creates a new lock holder.
    ///
    /// # Parameters
    ///
    /// - `exclusive`: Tells whether the holder has an exclusive lock or a shared lock.
    /// - `system_identifier`: PID of the process holding the lock.
    /// - `opaque_handle`: Host or process that is holding the lock.
    /// - `lock_offset`: Offset for the lock region.
    /// - `lock_length`: Length of the blocking region.
    ///
    /// # Returns
    ///
    /// Returns a new [`Nlm4Holder`] instance.
    pub fn new(
        exclusive: bool,
        system_identifier: i32,
        opaque_handle: OpaqueHandle,
        lock_offset: u64,
        lock_length: u64,
    ) -> Self {
        Nlm4Holder { exclusive, system_identifier, opaque_handle, lock_offset, lock_length }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_holder_succeeds() {
        let opaque_handle = OpaqueHandle::new(vec![1, 2, 3]);
        let system_id = 12345;
        let offset = 0;
        let length = 0;

        let lock = Nlm4Holder::new(true, system_id, opaque_handle, offset, length);

        assert_eq!(lock.opaque_handle.as_bytes(), &[1, 2, 3]);
        assert_eq!(lock.system_identifier, system_id);
        assert_eq!(lock.lock_offset, offset);
        assert_eq!(lock.lock_length, length);
    }
}
