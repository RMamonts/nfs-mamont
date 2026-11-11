//! Defines the NFSv4 COMMIT operation types, RFC 7530 §16.3.

use crate::consts::nfsv4::NFS4_VERIFIER_SIZE;

use super::{file, Error};

/// A write verifier (`verifier4`).
///
/// An [`NFS4_VERIFIER_SIZE`]-byte opaque identifier of a server instance,
/// used to detect a server reboot between a WRITE and a subsequent COMMIT.
pub struct Verifier(pub [u8; NFS4_VERIFIER_SIZE]);

/// COMMIT arguments (`COMMIT4args`).
pub struct Args {
    /// Position within the file at which the flush is to begin. A value of
    /// `0` means to flush starting at the beginning of the file.
    pub offset: u64,
    /// Number of bytes of data to flush. If `count` is `0`, a flush from
    /// the offset to the end of the file is done.
    pub count: u32,
}

/// Success result (`COMMIT4resok`).
pub struct Success {
    /// Write verifier provided by the server.
    pub writeverf: Verifier,
}

/// Fail result (`COMMIT4res`).
pub struct Fail {
    /// Error on failure.
    pub error: Error,
}
