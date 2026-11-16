//! Defines the NFSv4 DELEGPURGE operation types, RFC 7530 §16.5.

/// Client identifier (`clientid4`), RFC 7530 §16.5.2.
///
/// TODO: might be removed after <https://github.com/RMamonts/nfs-mamont/pull/461>
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ClientId(u64);

impl ClientId {
    /// Creates new instance of `ClientId`
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns raw `u64' identifier
    pub fn raw(&self) -> u64 {
        self.0
    }
}

/// DELEGPURGE arguments (`DELEGPURGE4args`).
pub struct Args {
    /// Client whose delegations are to be purged.
    pub clientid: ClientId,
}

/// Success result (`DELEGPURGE4res`).
pub struct Success;

/// Fail result (`DELEGPURGE4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
