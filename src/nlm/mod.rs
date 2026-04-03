//! Defines NLMv4 Network Lock Manager interface.
//!
//! This module contains types and structures for NLMv4 as defined in RFC 1813.

pub mod holder;
pub mod lock;
use num_derive::{FromPrimitive, ToPrimitive};

/// `Nlm4Stats` indicates the success or failure of a call.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ToPrimitive, FromPrimitive)]
#[allow(dead_code)]
pub enum Nlm4Stats {
    /// The call was successfully completed, and the lock was set.
    Granted = 0,
    /// For attempts to set a lock.
    /// If the client retries the call later, it may succeed.
    Denied = 1,
    /// The call failed because the server could not allocate the necessary resources.
    DeniedNolocks = 2,
    /// The request is queued.
    /// The server will issue an NLMPROC4_GRANTED callback to the client when the lock is granted.
    Blocked = 3,
    /// The call failed because the server is reestablishing old
    /// locks after a reboot and is not yet ready to resume normal service.
    DeniedGracePeriod = 4,
    /// The request could not be granted and blocking would cause a deadlock.
    Deadlock = 5,
    /// The call failed because the remote file system is read-only.
    Rofs = 6,
    /// The call failed because it uses an invalid file handle.
    /// This can happen if the file has been removed
    /// or if access to the file has been revoked on the server.
    StaleFh = 7,
    /// The call failed because it specified a length or offset
    /// that exceeds the range supported by the server.
    Fbig = 8,
    /// The call failed for some reason not already listed.
    /// The client should take this status as a strong hint not to retry the request.
    Failed = 9,
}

/// Opaque lock owner identifier (`oh`).
///
/// # Fields
/// - `owner_id`: the unique identifier of the lock owner.
#[derive(Debug)]
#[allow(dead_code)]
pub struct OpaqueHandle {
    opaque_handle: Vec<u8>,
}

impl OpaqueHandle {
    /// Creates a new instance of [`OpaqueHandle`].
    ///
    /// The field values correspond to the description in [`Nlm4Lock`].
    #[allow(dead_code)]
    pub fn new(oh: Vec<u8>) -> Self {
        OpaqueHandle { opaque_handle: oh }
    }

    /// Returns the underlying bytes of the opaque handle.
    #[allow(dead_code)]
    pub fn as_bytes(&self) -> &[u8] {
        &self.opaque_handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_handle_bytes() {
        let bytes = vec![0x01, 0x02];
        let oh = OpaqueHandle::new(bytes.clone());
        assert_eq!(oh.as_bytes(), bytes.as_slice());
    }
}
