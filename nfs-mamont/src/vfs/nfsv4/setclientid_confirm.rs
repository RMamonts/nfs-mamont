//! Defines the NFSv4 SETCLIENTID_CONFIRM operation types, RFC 7530 §16.34.

use crate::vfs::nfsv4::file;

/// SETCLIENTID_CONFIRM arguments (`SETCLIENTID_CONFIRM4args`).
pub struct Args {
    /// Client id returned by a prior SETCLIENTID operation.
    pub clientid: file::ClientId,
    /// Verifier returned by a prior SETCLIENTID operation.
    pub confirm: super::Verifier,
}

/// Success result (`SETCLIENTID_CONFIRM4res`).
pub struct Success;

/// Fail result (`SETCLIENTID_CONFIRM4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
