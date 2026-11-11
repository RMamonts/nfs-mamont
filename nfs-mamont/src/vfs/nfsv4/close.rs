//! Defines the NFSv4 CLOSE operation types, RFC 7530 §16.2.

use super::file;

/// CLOSE arguments (`CLOSE4args`).
pub struct Args {
    /// File handle of the object to be closed.
    pub file: file::Handle,
    /// Sequence id providing for the correct ordering of the operation.
    pub seqid: file::SeqId,
    /// State id of the open file description being closed.
    pub open_stateid: file::StateId,
}

/// Success result (`CLOSE4res`).
pub struct Success {
    /// State id associated with the open file description at the time of
    /// the close.
    pub open_stateid: file::StateId,
}

/// Fail result (`CLOSE4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
