use crate::nlm::{cookie::Cookie, Name, OpaqueHandle};
use crate::task::global::nlm::nlm_event::NlmEventHandler;

/// The wrapper needed to notify the client.
pub struct PendingGrant {
    /// Transaction identifier from the original blocking LOCK request;
    /// echoed back to the client in the GRANTED callback.
    pub cookie: Cookie,
    /// The channel for sending the callback.
    pub event_handler: Option<NlmEventHandler>,
}

impl PendingGrant {
    pub fn new(event_handler: Option<NlmEventHandler>, cookie: Cookie) -> Self {
        Self { event_handler, cookie }
    }
}

/// A held lock with full owner identity and state.
#[derive(Clone)]
pub struct ActiveLock {
    /// Name of the client host that owns the lock.
    pub caller_name: Name,
    /// PID of the process on the client that owns the lock.
    pub system_identifier: i32,
    /// `true` for exclusive lock, `false` for shared lock.
    pub exclusive: bool,
    /// Starting offset of the locked region (in bytes).
    pub offset: u64,
    /// Length of the locked region. A value of `0` means "to end-of-file".
    pub length: u64,
    /// Opaque handle identifying the lock owner (returned in TEST responses).
    pub opaque_handle: OpaqueHandle,
}

/// Equality compares only the unlock-identity fields
/// (`caller_name`, `system_identifier`, `offset`, `length`).
/// `exclusive` and `opaque_handle` are intentionally ignored —
/// UNLOCK identifies a lock by owner + range, not by mode or handle.
impl PartialEq for ActiveLock {
    fn eq(&self, other: &Self) -> bool {
        self.caller_name == other.caller_name
            && self.system_identifier == other.system_identifier
            && self.offset == other.offset
            && self.length == other.length
    }
}

/// A blocked (pending) lock request waiting to be granted.
pub struct PendingLock {
    /// Name of the client host that owns the lock.
    pub caller_name: Name,
    /// PID of the process on the client that owns the lock.
    pub system_identifier: i32,
    /// `true` for exclusive lock, `false` for shared lock.
    pub exclusive: bool,
    /// Starting byte offset of the requested lock region.
    pub offset: u64,
    /// Length of the requested lock region. `0` means to end-of-file.
    pub length: u64,
    /// Opaque handle identifying the lock owner (used in GRANTED callback).
    pub opaque_handle: OpaqueHandle,
    /// A wrapper for the cookie and a channel for sending it to the client.
    pub grant_notification: PendingGrant,
}

/// Converts a [`PendingLock`] reference into an [`ActiveLock`] by copying all shared fields.
/// The `cookie` field from the pending request is intentionally dropped,
/// as it is only relevant for the GRANTED callback and has no meaning for an active lock.
impl From<&PendingLock> for ActiveLock {
    fn from(lock: &PendingLock) -> Self {
        ActiveLock {
            caller_name: lock.caller_name.clone(),
            system_identifier: lock.system_identifier,
            exclusive: lock.exclusive,
            offset: lock.offset,
            length: lock.length,
            opaque_handle: lock.opaque_handle.clone(),
        }
    }
}

/// Equality compares all identity fields needed to match a `CANCEL` request.
/// `cookie` is excluded because it is a request-scoped transient identifier,
/// not an attribute of the lock itself.
impl PartialEq for PendingLock {
    fn eq(&self, other: &Self) -> bool {
        self.caller_name == other.caller_name
            && self.system_identifier == other.system_identifier
            && self.exclusive == other.exclusive
            && self.offset == other.offset
            && self.length == other.length
            && self.opaque_handle == other.opaque_handle
    }
}
