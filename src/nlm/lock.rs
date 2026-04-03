//! Defines NLMv4 LOCK procedure structures.
//!
//! Contains types for NLMPROC4_LOCK.

use crate::consts::nlm;
use crate::vfs;

/// Opaque lock owner identifier (`oh`).
///
/// # Fields
/// - `owner_id`: the unique identifier of the lock owner.
#[derive(Debug)]
#[allow(dead_code)]
pub struct OpaqueHandle {
    opaque_handle: Vec<u8>,
}

impl OpaqueHandle {
    /// Creates a new instance of [`OpaqueHandle`].
    ///
    /// The field values correspond to the description in [`Nlm4Lock`].
    #[allow(dead_code)]
    pub fn new(oh: Vec<u8>) -> Self {
        OpaqueHandle { opaque_handle: oh }
    }

    /// Returns the underlying bytes of the opaque handle.
    #[allow(dead_code)]
    pub fn as_bytes(&self) -> &[u8] {
        &self.opaque_handle
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

        if caller_name.len() > nlm::LM_MAXSTRLEN {
            return Err(format!("caller_name is too long (max {})", nlm::LM_MAXSTRLEN).into());
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
        let opaque_handle = OpaqueHandle::new(vec![1, 2, 3]);
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
            OpaqueHandle::new(vec![]),
            12345,
            0,
            0,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "caller_name must not be empty");
    }

    #[test]
    fn new_lock_fails_on_too_long_caller_name() {
        let result = Nlm4Lock::new(
            "a".repeat(nlm::LM_MAXSTRLEN + 1),
            Handle([0; NFS3_FHSIZE]),
            OpaqueHandle::new(vec![]),
            12345,
            0,
            0,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), format!("caller_name is too long (max {})", nlm::LM_MAXSTRLEN));
    }

    #[test]
    fn opaque_handle_bytes() {
        let bytes = vec![0x01, 0x02];
        let oh = OpaqueHandle::new(bytes.clone());
        assert_eq!(oh.as_bytes(), bytes.as_slice());
    }
}
