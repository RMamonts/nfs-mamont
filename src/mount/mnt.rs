//! Defines Mount version 3 Mnt interface (Procedure 1).
//!
//! as defined in RFC 1813 section 5.2.1.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.1>.

use async_trait::async_trait;

use num_derive::{FromPrimitive, ToPrimitive};

use crate::rpc::AuthFlavor;
use crate::vfs::file;

#[derive(ToPrimitive, FromPrimitive)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
/// Possible MOUNT errors
pub enum Fail {
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct Success {
    /// The file handle for the mounted directory.
    /// This file handle may be used in the NFS protocol.
    pub file_handle: file::Handle,
    /// Vector of RPC authentication flavors that are supported with
    /// the client's use of the file handle (or any file handles derived from it)
    pub auth_flavors: Vec<AuthFlavor>,
}

/// Arguments for the Mount operation, containing the path to be mounted.
#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Clone))]
pub struct Args {
    /// a server pathname of a directory
    pub dirpath: file::Path,
}

#[async_trait]
pub trait Mnt {
    /// Maps a pathname on the server to a NFS version 3 protocol file handle.
    ///
    /// # Parameters:
    /// * `args` --- the arguments for the Mount operation.
    ///
    /// This procedure also results in the server adding a new entry
    /// to its mount list recording that this client has mounted the directory.
    async fn mnt(&self, args: Args) -> Result<Success, Fail>;
}
