//! Defines NLMv4 file share structures.
//!
//! Contains [`Nlm4Share`], [`FileSharingMode`] and [`FileSharingAccess`] types for DOS file sharing.

use std::io::Error;

use crate::consts::nlm;
use crate::vfs;

use super::OpaqueHandle;

/// DOS-style file sharing mode.
///
/// Defines what operations other clients are prohibited from performing.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FileSharingMode {
    /// Other clients may perform any operation.
    None = 0,
    /// Other clients are prohibited from reading the file.
    Read = 1,
    /// Other clients are prohibited from writing to the file.
    Write = 2,
    /// Other clients are prohibited from reading and writing.
    ReadWrite = 3,
}

/// DOS-style file sharing access mode.
///
/// Defines what operations the requesting client is allowed to perform.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FileSharingAccess {
    /// Client has no access to the file.
    None = 0,
    /// Client may read the file.
    Read = 1,
    /// Client may write to the file.
    Write = 2,
    /// Client may read and write the file.
    ReadWrite = 3,
}

/// This structure is used to support DOS file sharing.
pub struct Nlm4Share {
    /// Name of the client host making the lock request.
    pub caller_name: String,
    /// Handle to the file to share.
    pub file_handle: vfs::file::Handle,
    /// Host or process that is making the request.
    pub opaque_handle: OpaqueHandle,
    /// Specifies operations prohibited to other clients.
    pub fsh4_mode: FileSharingMode,
    /// Specifies operations allowed to the requesting client.
    pub fsh4_access: FileSharingAccess,
}

impl Nlm4Share {
    /// Creates a new file share request.
    ///
    /// # Parameters
    ///
    /// - `caller_name`: Name of the client host making the lock request.
    /// - `file_handle`: Handle to the file to share.
    /// - `opaque_handle`: Host or process that is making the request.
    /// - `fsh4_mode`: Specifies operations prohibited to other clients.
    /// - `fsh4_access`: Specifies operations allowed to the requesting client.
    ///
    /// # Returns
    ///
    /// Returns a new [`Nlm4Share`] instance if the request is valid.
    ///
    /// # Errors
    ///
    /// Returns `Err` with a text message if:
    ///
    /// - `caller_name` is empty.
    /// - `caller_name` is longer than `LM_MAXSTRLEN`.
    pub fn new(
        caller_name: String,
        file_handle: vfs::file::Handle,
        opaque_handle: OpaqueHandle,
        fsh4_mode: FileSharingMode,
        fsh4_access: FileSharingAccess,
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

        Ok(Nlm4Share { caller_name, file_handle, opaque_handle, fsh4_mode, fsh4_access })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::consts::nfsv3::NFS3_FHSIZE;
    use crate::vfs::file::Handle;

    #[test]
    fn new_share_succeeds() {
        let caller_name = "host".to_string();
        let file_handle = Handle([0; NFS3_FHSIZE]);
        let opaque_handle = OpaqueHandle::new(vec![1, 2, 3]);
        let fsh4_mode = FileSharingMode::Read;
        let fsh4_access = FileSharingAccess::ReadWrite;

        let lock =
            Nlm4Share::new(caller_name.clone(), file_handle, opaque_handle, fsh4_mode, fsh4_access)
                .unwrap();

        assert_eq!(lock.caller_name, caller_name);
        assert_eq!(lock.file_handle.0, [0; NFS3_FHSIZE]);
        assert_eq!(lock.opaque_handle.as_bytes(), &[1, 2, 3]);
        assert_eq!(lock.fsh4_access, fsh4_access);
        assert_eq!(lock.fsh4_mode, fsh4_mode);
    }
}
