//! Defines Mount version 3 [`Dump`] interface (Procedure 2).
//!
//! as defined in RFC 1813 section 5.2.2.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.2>.
use crate::xdr;

use super::MountEntry;

/// Success result.
pub struct Success {
    /// List of remotely mounted file systems.
    /// Contains one entry for each client host name and directory pair.
    /// The list is derived from a list maintained on the server
    /// of clients that have requested file handles with the MNT procedure.
    pub mount_list: Vec<MountEntry>,
}

impl xdr::XDRSize for Success {
    fn xdr_size(&self) -> usize {
        self.mount_list.iter().map(|entry| entry.xdr_size() + Self::INTEGER).sum::<usize>()
            + Self::INTEGER
    }
}

#[trait_variant::make(Send)]
pub trait Dump {
    /// Retrieves the list of remotely mounted file systems.
    ///
    /// There are no MOUNT protocol errors which can be returned from this procedure.
    async fn dump(&self) -> Success;
}
