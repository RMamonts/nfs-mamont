//! Defines Mount version 3 [`Dump`] interface (Procedure 1).
//!
//! as defined in RFC 1813 section 5.2.1.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.1>.

#![allow(dead_code)]

use async_trait::async_trait;

use super::{MountEntry, Error};

/// Success result.
pub struct Success {
    /// List of remotely mounted file systems.
    /// Contains one entry for each client host name and directory pair.
    /// The list is derived from a list maintained on the server
    /// of clients that have requested file handles with the MNT procedure.
    mount_list: Vec<MountEntry>,
}

// TODO (artemiipatov: fail case)
type Result = std::result::Result<Success, Error>;

/// Defines callback to pass [`Dump::dump`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(result: Result);
}

#[async_trait]
pub trait Dump {
    /// Retrieves the list of remotely mounted file systems.
    async fn dump(&self, promise: impl Promise);
}