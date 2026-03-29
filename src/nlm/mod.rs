//! Defines NLMv4 Network Lock Manager interface.
//!
//! This module contains types and structures for NLMv4 as defined in RFC 1813.

use crate::vfs;
use num_derive::{FromPrimitive, ToPrimitive};

#[derive(Debug, Copy, Clone, PartialEq, Eq, ToPrimitive, FromPrimitive)]
#[allow(dead_code)]
pub enum Nlm4Stats {
    /// The call was successfully completed, and the lock was set.
    Granted = 0,
    /// For attempts to set a lock.
    /// If the client retries the call later, it may succeed.
    Denied = 1,
    /// The call failed because the server could not allocate the necessary resources.
    DeniedNolocks = 2,
    /// The request is queued.
    /// The server will issue an NLMPROC4_GRANTED callback to the client when the lock is granted.
    Blocked = 3,
    /// The call failed because the server is reestablishing old
    /// locks after a reboot and is not yet ready to resume normal service.
    DeniedGracePeriod = 4,
    /// The request could not be granted and blocking would cause a deadlock.
    Deadlock = 5,
    /// The call failed because the remote file system is read-only.
    Rofs = 6,
    /// The call failed because it uses an invalid file handle.
    /// This can happen if the file has been removed
    /// or if access to the file has been revoked on the server.
    StaleFh = 7,
    /// The call failed because it specified a length or offset
    /// that exceeds the range supported by the server.
    Fbig = 8,
    /// The call failed for some reason not already listed.
    /// The client should take this status as a strong hint not to retry the request.
    Failed = 9,
}

/// Opaque lock owner identifier (`oh`).
#[derive(Debug)]
#[allow(dead_code)]
pub struct OpaqueHandle(Vec<u8>);

impl OpaqueHandle {
    /// Returns the underlying bytes of the opaque handle.
    #[allow(dead_code)]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// This structure describes a lock request.
///
/// # Fields
/// - `caller_name`: host that is making the request.
/// - `file_handle`: file to lock.
/// - `opaque_handle`: host or process that is making the request
/// - `system_identifier`: process that is making the request.
/// - `lock_offset`: offset for the lock region.
/// - `lock_length`: length of the blocking region. A l_len of 0 means "to end of file".
#[derive(Debug)]
#[allow(dead_code)]
pub struct Nlm4Lock {
    caller_name: String,
    file_handle: vfs::file::Handle,
    opaque_handle: OpaqueHandle,
    system_identifier: i32,
    lock_offset: u64,
    lock_length: u64,
}

#[allow(dead_code)]
impl Nlm4Lock {
    /// Creates a new instance of [`Nlm4Lock`] with the specified parameters.
    ///
    /// The field values correspond to the description in [`Nlm4Lock`].
    ///
    /// # Errors
    /// Returns `Err` with a text message if:
    /// - `caller_name` is empty.
    pub fn new(
        caller_name: String,
        file_handle: vfs::file::Handle,
        opaque_handle: OpaqueHandle,
        system_identifier: i32,
        lock_offset: u64,
        lock_length: u64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if caller_name.is_empty() {
            return Err("caller_name must not be empty".into());
        }
        Ok(Nlm4Lock {
            caller_name,
            file_handle,
            opaque_handle,
            system_identifier,
            lock_offset,
            lock_length,
        })
    }

    /// Returns the host name of the client.
    ///
    /// See the description of the `caller_name` field in [`Nlm4Lock`].
    pub fn caller_name(&self) -> &str {
        &self.caller_name
    }

    /// Returns the file handle of the client.
    ///
    /// See the description of the `file_handle` field in [`Nlm4Lock`].
    pub fn file_handle(&self) -> &vfs::file::Handle {
        &self.file_handle
    }

    /// Returns the opaque handle of the client.
    ///
    /// See the description of the `opaque_handle` field in [`Nlm4Lock`].
    pub fn opaque_handle(&self) -> &OpaqueHandle {
        &self.opaque_handle
    }

    /// Returns the system identifier (`svid`).
    ///
    /// This is a copy of the original value.
    /// See the `system_identifier` field in [`Nlm4Lock`].
    pub fn system_identifier(&self) -> i32 {
        self.system_identifier
    }

    /// Returns the lock offset.
    ///
    /// This is a copy of the original value.
    /// See the `lock_offset` field in [`Nlm4Lock`].
    pub fn lock_offset(&self) -> u64 {
        self.lock_offset
    }

    /// Returns the lock length.
    ///
    /// This is a copy of the original value.
    /// See the `lock_length` field in [`Nlm4Lock`].
    pub fn lock_length(&self) -> u64 {
        self.lock_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::file::Handle;

    use crate::consts::nfsv3::NFS3_FHSIZE;

    #[test]
    fn new_lock_succeeds() {
        let caller_name = "host".to_string();
        let file_handle = Handle([0; NFS3_FHSIZE]);
        let opaque_handle = OpaqueHandle(vec![1, 2, 3]);
        let system_id = 12345;
        let offset = 0;
        let length = 0;

        let lock = Nlm4Lock::new(
            caller_name.clone(),
            file_handle,
            opaque_handle,
            system_id,
            offset,
            length,
        )
        .unwrap();

        assert_eq!(lock.caller_name(), caller_name);
        assert_eq!(lock.file_handle().0, [0; NFS3_FHSIZE]);
        assert_eq!(lock.opaque_handle().as_bytes(), &[1, 2, 3]);
        assert_eq!(lock.system_identifier(), system_id);
        assert_eq!(lock.lock_offset(), offset);
        assert_eq!(lock.lock_length(), length);
    }

    #[test]
    fn new_lock_fails_on_empty_caller_name() {
        let result = Nlm4Lock::new(
            "".to_string(),
            Handle([0; NFS3_FHSIZE]),
            OpaqueHandle(vec![]),
            12345,
            0,
            0,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "caller_name must not be empty");
    }

    #[test]
    fn opaque_handle_bytes() {
        let bytes = vec![0x01, 0x02];
        let oh = OpaqueHandle(bytes.clone());
        assert_eq!(oh.as_bytes(), bytes.as_slice());
    }
}
