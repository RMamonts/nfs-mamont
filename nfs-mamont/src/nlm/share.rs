//! Defines NLMv4 file share structures.
//!
//! Contains [`Nlm4Share`], [`FileSharingMode`] and [`FileSharingAccess`] types for DOS file sharing.

use crate::vfs;

use super::{Name, OpaqueHandle};

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
    pub caller_name: Name,
    /// Handle to the file to share.
    pub file_handle: vfs::file::Handle,
    /// Host or process that is making the request.
    pub opaque_handle: OpaqueHandle,
    /// Specifies operations prohibited to other clients.
    pub fsh4_mode: FileSharingMode,
    /// Specifies operations allowed to the requesting client.
    pub fsh4_access: FileSharingAccess,
}
