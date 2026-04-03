use super::OpaqueHandle;

/// This structure indicates the holder of a lock.
///
/// # Fields
/// - `exclusive`: tells whether the holder has an exclusive lock or a shared lock.
/// - `system_identifier`: process that is holding the lock.
/// - `opaque_handle`: host or process that is holding the lock.
/// - `lock_offset`: offset for the lock region.
/// - `lock_length`: length of the blocking region. A l_len of 0 means "to end of file".
#[allow(dead_code)]
pub struct Nlm4Holder {
    exclusive: bool,
    system_identifier: i32,
    opaque_handle: OpaqueHandle,
    lock_offset: u64,
    lock_length: u64,
}

#[allow(dead_code)]
impl Nlm4Holder {
    /// Creates a new instance of [`Nlm4Holder`] with the specified parameters.
    ///
    /// The field values correspond to the description in [`Nlm4Holder`].
    pub fn new(
        exclusive: bool,
        system_identifier: i32,
        opaque_handle: OpaqueHandle,
        lock_offset: u64,
        lock_length: u64,
    ) -> Self {
        Nlm4Holder { exclusive, system_identifier, opaque_handle, lock_offset, lock_length }
    }

    /// Returns the exclusive flag.
    ///
    /// This is a copy of the original value.
    /// See the description of the `exclusive ` field in [`Nlm4Holder`].
    pub fn exclusive(&self) -> bool {
        self.exclusive
    }

    /// Returns the opaque handle of the client.
    ///
    /// See the description of the `opaque_handle` field in [`Nlm4Holder`].
    pub fn opaque_handle(&self) -> &OpaqueHandle {
        &self.opaque_handle
    }

    /// Returns the system identifier (`svid`).
    ///
    /// This is a copy of the original value.
    /// See the `system_identifier` field in [`Nlm4Holder`].
    pub fn system_identifier(&self) -> i32 {
        self.system_identifier
    }

    /// Returns the lock offset.
    ///
    /// This is a copy of the original value.
    /// See the `lock_offset` field in [`Nlm4Holder`].
    pub fn lock_offset(&self) -> u64 {
        self.lock_offset
    }

    /// Returns the lock length.
    ///
    /// This is a copy of the original value.
    /// See the `lock_length` field in [`Nlm4Holder`].
    pub fn lock_length(&self) -> u64 {
        self.lock_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_holderer_succeeds() {
        let opaque_handle = OpaqueHandle::new(vec![1, 2, 3]);
        let system_id = 12345;
        let offset = 0;
        let length = 0;

        let lock = Nlm4Holder::new(true, system_id, opaque_handle, offset, length);

        assert_eq!(lock.opaque_handle().as_bytes(), &[1, 2, 3]);
        assert_eq!(lock.system_identifier(), system_id);
        assert_eq!(lock.lock_offset(), offset);
        assert_eq!(lock.lock_length(), length);
    }
}
