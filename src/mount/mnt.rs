//! Defines Mount version 3 [`Mnt`] interface (Procedure 1).
//!
//! as defined in RFC 1813 section 5.2.1.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.1>.

use async_trait::async_trait;

use super::{DirPath, Error, Handle};

/// Authentication flavors.
#[derive(Debug)]
pub enum AuthFlavor {
    None,
    Unix,
    Short,
    Des,
    Kerb,
}

/// Success result.
pub struct Success {
    /// The file handle for the mounted directory.
    /// This file handle may be used in the NFS protocol.
    pub file_handle: Handle,
    /// Vector of RPC authentication flavors that are supported with
    /// the client's use of the file handle (or any file handles derived from it)
    pub auth_flavors: Vec<AuthFlavor>,
}

pub type Result = std::result::Result<Success, Error>;

/// Defines callback to pass [`Mnt::mnt`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(result: Result);
}

#[async_trait]
pub trait Mnt {
    /// Maps a pathname on the server to a NFS version 3 protocol file handle.
    ///
    /// # Parameters:
    /// * `dirpath` --- a server pathname of a directory.
    /// 
    /// This procedure also results in the server adding a new entry 
    /// to its mount list recording that this client has mounted the directory.
    async fn mnt(&self, dirpath: DirPath, promise: impl Promise);
}
