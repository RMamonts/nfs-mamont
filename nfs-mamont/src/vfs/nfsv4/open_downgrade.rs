//! Defines the NFSv4 OPEN_DOWNGRADE operation types, RFC 7530 §16.19.

use super::file;

/// OPEN_DOWNGRADE arguments (`OPEN_DOWNGRADE4args`).
pub struct Args {
    /// State id of the open being downgraded.
    pub open_stateid: file::StateId,
    /// Sequence id providing for the correct ordering of the operation.
    pub seqid: file::SeqId,
    /// Bitmask of the access operations the client still requests.
    pub share_access: u32,
    /// Bitmask of the access operations the client wants to deny.
    pub share_deny: u32,
}

/// Success result (`OPEN_DOWNGRADE4resok`).
pub struct Success {
    /// State id of the downgraded open.
    pub open_stateid: file::StateId,
}

/// Fail result (`OPEN_DOWNGRADE4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
