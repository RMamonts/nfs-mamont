//! Defines NLMv4 basic lock structures.
//!
//! Contains [`Nlm4Lock`] and [`OpaqueHandle`] types used by lock procedures.

use std::io::Error;

#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;

use crate::consts::nlm;
use crate::vfs;

use super::OpaqueHandle;

/// This structure describes a lock request.
#[cfg_attr(feature = "arbitrary", derive(Clone, Debug, PartialEq))]
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
    /// Length of the blocking region. An l_len of 0 means "to end of file".
    pub lock_length: u64,
}

#[cfg(feature = "arbitrary")]
impl Arbitrary<'_> for Nlm4Lock {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let max_len = u.int_in_range(1..=nlm::LM_MAXSTRLEN)?;
        let mut caller_name = String::new();

        for _ in 0..max_len {
            match u.int_in_range(0u32..=25u32) {
                Ok(idx) => {
                    caller_name.push((b'a' + idx as u8) as char);
                }
                Err(_) => break,
            }
        }
        Ok(Nlm4Lock {
            caller_name,
            file_handle: u.arbitrary()?,
            opaque_handle: u.arbitrary()?,
            system_identifier: u.arbitrary()?,
            lock_offset: u.arbitrary()?,
            lock_length: u.arbitrary()?,
        })
    }
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
    ///
    /// # Errors
    ///
    /// Returns `Err` with a text message if:
    ///
    /// - `caller_name` is empty.
    /// - `caller_name` is longer than [`LM_MAXSTRLEN`](nlm::LM_MAXSTRLEN).
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
    use crate::consts::nfsv3::NFS3_FHSIZE;
    use crate::consts::nlm::OPAQUE_HANDLE_SIZE;
    use crate::vfs::file::Handle;

    use super::{nlm::LM_MAXSTRLEN, Nlm4Lock, OpaqueHandle};

    #[test]
    fn new_lock_succeeds() {
        let caller_name = "host".to_string();
        let fh = [0; NFS3_FHSIZE];
        let file_handle = Handle(fh);
        let oh = [1; OPAQUE_HANDLE_SIZE].to_vec();
        let oh_verf = oh.clone();
        let opaque_handle = OpaqueHandle::new(oh).unwrap();
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
        assert_eq!(lock.file_handle.0, fh);
        assert_eq!(lock.opaque_handle.as_bytes(), oh_verf.as_slice());
        assert_eq!(lock.system_identifier, system_id);
        assert_eq!(lock.lock_offset, offset);
        assert_eq!(lock.lock_length, length);
    }

    #[test]
    fn new_lock_fails_on_empty_caller_name() {
        let result = Nlm4Lock::new(
            "".to_string(),
            Handle([0; NFS3_FHSIZE]),
            OpaqueHandle::new([1; OPAQUE_HANDLE_SIZE].to_vec()).unwrap(),
            12345,
            0,
            0,
        );
        assert!(result.is_err());
    }

    #[test]
    fn new_lock_fails_on_too_long_caller_name() {
        let result = Nlm4Lock::new(
            "a".repeat(LM_MAXSTRLEN + 1),
            Handle([0; NFS3_FHSIZE]),
            OpaqueHandle::new([0; OPAQUE_HANDLE_SIZE].to_vec()).unwrap(),
            12345,
            0,
            0,
        );
        assert!(result.is_err());
    }
}
