//! Defines the NFSv4 OPEN_CONFIRM operation types, RFC 7530 §16.18.

use super::file;

/// OPEN_CONFIRM arguments (`OPEN_CONFIRM4args`).
pub struct Args {
    /// State id of the open being confirmed.
    pub open_stateid: file::StateId,
    /// Sequence id confirming the open.
    pub seqid: file::SeqId,
}

/// Success result (`OPEN_CONFIRM4resok`).
pub struct Success {
    /// State id of the confirmed open.
    pub open_stateid: file::StateId,
}

/// Fail result (`OPEN_CONFIRM4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
