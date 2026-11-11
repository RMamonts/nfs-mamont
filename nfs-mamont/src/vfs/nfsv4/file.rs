use crate::consts::nfsv4::NFS4_FHSIZE;

/// A file handle (`nfs_fh4`).
///
/// Corresponds to the file handle from RFC 7530. It is an opaque byte array
/// of at most [`NFS4_FHSIZE`] bytes, interpreted only by the server.
///
/// TODO: https://github.com/RMamonts/nfs-mamont/issues/496
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
