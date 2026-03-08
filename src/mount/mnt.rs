//! Defines Mount version 3 Mnt interface (Procedure 1).
//!
//! as defined in RFC 1813 section 5.2.1.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.1>.

use num_derive::{FromPrimitive, ToPrimitive};

use crate::rpc::AuthFlavor;
use crate::vfs::file;

#[derive(ToPrimitive, FromPrimitive)]
/// Possible MOUNT errors
pub enum MntError {
    /// Not owner
    Perm = 1,
    /// No such file or directory
    NoEnt = 2,
    /// I/O error
    Io = 5,
    /// Permission denied
    Access = 13,
    /// Not a directory
    NoDir = 20,
    /// Invalid argument
    Inval = 22,
    /// Filename too long
    NameTooLong = 63,
    /// Operation not supported
    NotSupp = 10004,
    /// A failure on the server
    ServerFault = 10006,
}

/// Success result.
pub struct Success {
    /// The file handle for the mounted directory.
    /// This file handle may be used in the NFS protocol.
    pub file_handle: file::Handle,
    /// Vector of RPC authentication flavors that are supported with
    /// the client's use of the file handle (or any file handles derived from it)
    pub auth_flavors: Vec<AuthFlavor>,
}

pub type Result = std::result::Result<Success, MntError>;

/// Arguments for the Mount operation, containing the path to be mounted.
#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct MountArgs(pub file::Path);
