//! Defines the NFSv4 DELEGRETURN operation types, RFC 7530 §16.6.

use super::file::StateId;

/// DELEGRETURN arguments (`DELEGRETURN4args`).
pub struct Args {
    /// Stateid of the delegation to return.
    pub deleg_stateid: StateId,
}

/// Success result (`DELEGRETURN4res`).
pub struct Success;

/// Fail result (`DELEGRETURN4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
