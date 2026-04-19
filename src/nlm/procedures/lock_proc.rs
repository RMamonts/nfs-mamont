use crate::nlm::lock::Nlm4Lock;
use crate::nlm::Nlm4Stats;
use crate::vfs::read_dir::Cookie;

/// Defines the information needed to request a lock on a server.
pub struct Nlm4LockArgs {
    /// Transaction identifier for matching request/response.
    pub cookie: Cookie,
    /// `True` if the client wishes the procedure call to block until the lock can be granted.
    /// A `false` value will cause the procedure call to return immediately if the lock cannot be granted.
    pub block: bool,
    /// `True` for exclusive lock, `false` for shared lock.
    pub exclusive: bool,
    /// Lock details (caller name, file handle, offset, length).
    pub lock: Nlm4Lock,
    /// `True` if the client is attempting to reclaim a lock held by an NLM which has been restarted (due to a server crash, and so on).
    pub reclaim: bool,
    /// It is the state value supplied by the local Network Status Monitor Protocol.
    pub state: u32,
}

/// NLM LOCK result.
///
/// Returned by [`NLMPROC4_LOCK`](crate::consts::nlm::NLMPROC4_LOCK) procedure.
pub struct Nlm4LockRes {
    /// Transaction identifier for matching request/response.
    pub cookie: Cookie,
    /// Status code (Granted, Denied, etc.).
    pub stat: Nlm4Stats,
}
