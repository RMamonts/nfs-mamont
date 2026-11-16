//! Defines the NFSv4 RENEW operation types, RFC 7530 §16.28.

use crate::vfs::nfsv4::file;

/// RENEW arguments (`RENEW4args`).
pub struct Args {
    /// Client whose leases are to be renewed.
    pub clientid: file::ClientId,
}

/// Success result (`RENEW4res`).
pub struct Success;

/// Fail result (`RENEW4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
