//! Defines NLMv3 Network Lock Manager interface --- [`Nlm`].

use num_derive::{FromPrimitive, ToPrimitive};

#[derive(Debug, Copy, Clone, PartialEq, Eq, ToPrimitive, FromPrimitive)]
pub enum nlm4_stats {
    /// The call was successfully completed, and the lock was set.
    NLM4_GRANTED = 0,
    /// For attempts to set a lock.
    /// If the client retries the call later, it may succeed.
    NLM4_DENIED = 1,
    /// The call failed because the server could not allocate the necessary resources.
    NLM4_DENIED_NOLOCKS = 2,
    /// The request is queued.
    /// The server will issue an NLMPROC4_GRANTED callback to the client when the lock is granted.
    NLM4_BLOCKED = 3,
    /// The call failed because the server is reestablishing old
    /// locks after a reboot and is not yet ready to resume normal service.
    NLM4_DENIED_GRACE_PERIOD = 4,
    /// The request could not be granted and blocking would cause a deadlock.
    NLM4_DEADLACK = 5,
    /// The call failed because the remote file system is read-only.
    NLM4_ROFS = 6,
    /// The call failed because it uses an invalid file handle.
    /// This can happen if the file has been removed
    /// or if access to the file has been revoked on the server.
    NLM4_STALE_FH = 7,
    /// The call failed because it specified a length or offset
    /// that exceeds the range supported by the server.
    NLM4_FBIG = 8,
    /// The call failed for some reason not already listed.
    /// The client should take this status as a strong hint not to retry the request.
    NLM4_FAILED = 9,
}
