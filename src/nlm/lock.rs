//! Defines NLMv4 basic lock structures.
//!
//! Contains [`Nlm4Lock`] and [`OpaqueHandle`] types used by lock procedures.

use super::OpaqueHandle;
use crate::consts::nlm;
use crate::vfs;
use std::io::Error;

/// This structure describes a lock request.
pub struct Nlm4Lock {
    /// Name of the client host making the lock request.
    pub caller_name: String,
    /// Handle to the file to lock.
    pub file_handle: vfs::file::Handle,
    /// Host or process that is making the request.
    pub opaque_handle: OpaqueHandle,
    /// PID of the process making the request.
    pub system_identifier: i32,
    /// Offset for the lock region.
    pub lock_offset: u64,
    /// Length of the blocking region. A l_len of 0 means "to end of file".
    pub lock_length: u64,
}

impl Nlm4Lock {
    /// Creates a new lock request.
    ///
    /// # Errors
    /// Returns `Err` with a text message if:
    /// - `caller_name` is empty.
    /// - `caller_name` is longer than `LM_MAXSTRLEN`.
    pub fn new(
        caller_name: String,
        file_handle: vfs::file::Handle,
        opaque_handle: OpaqueHandle,
        system_identifier: i32,
        lock_offset: u64,
        lock_length: u64,
    ) -> Result<Self, Error> {
        if caller_name.is_empty() {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "caller_name must not be empty",
            ));
        }

        if caller_name.len() > nlm::LM_MAXSTRLEN {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("caller_name is too long (max {})", nlm::LM_MAXSTRLEN),
            ));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::consts::nfsv3::NFS3_FHSIZE;
    use crate::vfs::file::Handle;

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

        assert_eq!(lock.caller_name, caller_name);
        assert_eq!(lock.file_handle.0, [0; NFS3_FHSIZE]);
        assert_eq!(lock.opaque_handle.as_bytes(), &[1, 2, 3]);
        assert_eq!(lock.system_identifier, system_id);
        assert_eq!(lock.lock_offset, offset);
        assert_eq!(lock.lock_length, length);
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
    }
}
