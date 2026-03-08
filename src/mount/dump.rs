//! Defines Mount version 3 DUMP procedure data types (Procedure 2).
//!
//! as defined in RFC 1813 section 5.2.2.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.2>.

use super::MountEntry;

/// Success result.
pub struct Success {
    /// List of remotely mounted file systems.
    /// Contains one entry for each client host name and directory pair.
    /// The list is derived from a list maintained on the server
    /// of clients that have requested file handles with the MNT procedure.
    pub mount_list: Vec<MountEntry>,
}
