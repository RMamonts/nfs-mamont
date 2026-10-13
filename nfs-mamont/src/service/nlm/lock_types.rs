use crate::nlm::{cookie::Cookie, Name, OpaqueHandle};

/// A blocked (pending) lock request waiting to be granted.
#[derive(Clone)]
pub struct Lock {
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
    /// The cookie from the original blocking LOCK request.
    /// TODO: Needed for NLMPROC4_GRANTED callback (#267).
    pub cookie: Cookie,
}

/// Equality compares all identity fields needed to match a `CANCEL` request.
/// `cookie` is excluded because it is a request-scoped transient identifier,
/// not an attribute of the lock itself.
impl PartialEq for Lock {
    fn eq(&self, other: &Self) -> bool {
        self.caller_name == other.caller_name
            && self.system_identifier == other.system_identifier
            && self.exclusive == other.exclusive
            && self.offset == other.offset
            && self.length == other.length
            && self.opaque_handle == other.opaque_handle
    }
}
