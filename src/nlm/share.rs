//! Defines NLMv4 file share structures.
//!
//! Contains [`Nlm4Share`], [`Fsh4Mode`] and [`Fsh4Access`] types for DOS file sharing.

use super::OpaqueHandle;
use crate::consts::nlm;
use crate::vfs;

/// DOS-style file sharing mode.
///
/// Defines what operations other clients are prohibited from performing.
#[derive(Debug)]
#[allow(dead_code)]
enum FileSharingMode {
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
#[derive(Debug)]
#[allow(dead_code)]
enum FileSharingAccess {
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
///
/// # Fields
/// - `caller_name`: host that is making the request.
/// - `file_handle`: file to be operated on.
/// - `opaque_handle`: host or process that is making the request.
/// - `fsh4_mode`: specifies operations prohibited to other clients.
/// - `fsh4_access`: specifies operations allowed to the requesting client.
#[allow(dead_code)]
struct Nlm4Share {
    caller_name: String,
    file_handle: vfs::file::Handle,
    opaque_handle: OpaqueHandle,
    fsh4_mode: FileSharingMode,
    fsh4_access: FileSharingAccess,
}

#[allow(dead_code)]
impl Nlm4Share {
    /// Creates a new instance of [`Nlm4Share`] with the specified parameters.
    ///
    /// The field values correspond to the description in [`Nlm4Share`].
    ///
    /// # Errors
    /// Returns `Err` with a text message if:
    /// - `caller_name` is empty.
    /// - `caller_name` is longer than `LM_MAXSTRLEN`.
    pub fn new(
        caller_name: String,
        file_handle: vfs::file::Handle,
        opaque_handle: OpaqueHandle,
        fsh4_mode: FileSharingMode,
        fsh4_access: FileSharingAccess,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if caller_name.is_empty() {
            return Err("caller_name must not be empty".into());
        }

        if caller_name.len() > nlm::LM_MAXSTRLEN {
            return Err(format!("caller_name is too long (max {})", nlm::LM_MAXSTRLEN).into());
        }

        Ok(Nlm4Share { caller_name, file_handle, opaque_handle, fsh4_mode, fsh4_access })
    }

    /// Returns the host name of the client.
    ///
    /// See the description of the `caller_name` field in [`Nlm4Share`].
    pub fn caller_name(&self) -> &str {
        &self.caller_name
    }

    /// Returns the file handle of the client.
    ///
    /// See the description of the `file_handle` field in [`Nlm4Share`].
    pub fn file_handle(&self) -> &vfs::file::Handle {
        &self.file_handle
    }

    /// Returns the opaque handle of the client.
    ///
    /// See the description of the `opaque_handle` field in [`Nlm4Share`].
    pub fn opaque_handle(&self) -> &OpaqueHandle {
        &self.opaque_handle
    }

    /// Returns the file sharing mode.
    ///
    /// See the description of the `fsh4_mode` field in [`Nlm4Share`].
    pub fn fsh4_mode(&self) -> &FileSharingMode {
        &self.fsh4_mode
    }

    /// Returns the file sharing access mode.
    ///
    /// See the description of the `fsh4_access` field in [`Nlm4Share`].
    pub fn fsh4_access(&self) -> &FileSharingAccess {
        &self.fsh4_access
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

        assert_eq!(lock.caller_name(), caller_name);
        assert_eq!(lock.file_handle().0, [0; NFS3_FHSIZE]);
        assert_eq!(lock.opaque_handle().as_bytes(), &[1, 2, 3]);
        assert_eq!(*lock.fsh4_access(), fsh4_access);
        assert_eq!(*lock.fsh4_mode(), fsh4_mode);
    }
}
