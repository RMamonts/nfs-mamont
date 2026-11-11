use crate::consts::nfsv4::{NFS4_FHSIZE, NFS4_OTHER_SIZE};

/// A file handle (`nfs_fh4`).
///
/// Corresponds to the file handle from RFC 7530. It is an opaque byte array
/// of at most [`NFS4_FHSIZE`] bytes, interpreted only by the server.
///
/// TODO: <https://github.com/RMamonts/nfs-mamont/issues/496>
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Handle([u8; NFS4_FHSIZE]);

impl Handle {
    /// Creates a new [`Handle`] from the raw bytes as received from the NFS
    /// client, validating its length.
    ///
    /// # Errors
    ///
    /// Returns an error if `raw.len() > NFS4_FHSIZE`.
    pub fn new(raw: [u8; NFS4_FHSIZE]) -> Self {
        Handle(raw)
    }

    /// Returns the raw bytes of the handle.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// A sequence identifier (`seqid4`).
///
/// Used to order operations on the same state and detect duplicate or
/// out-of-order requests. See RFC 7530 §2.2.15.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct SeqId(u32);

impl SeqId {
    /// Builds a sequence id from its raw value.
    pub fn new(raw: u32) -> Self {
        SeqId(raw)
    }

    /// Returns the raw value of the sequence id.
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// A state identifier (`stateid4`).
///
/// Used to identify a particular state of the file opened by a client, such
/// as an open file description or a byte-range lock. See RFC 7530 §2.2.16.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StateId {
    /// Monotonically increasing sequence id for the state.
    pub seqid: SeqId,
    /// Opaque [`crate::consts::nfsv4::NFS4_OTHER_SIZE`]-byte value identifying
    /// the state.
    pub other: [u8; NFS4_OTHER_SIZE],
}
