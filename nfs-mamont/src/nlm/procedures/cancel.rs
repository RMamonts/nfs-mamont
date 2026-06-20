//! NLMv4 CANCEL procedure types.
//!
//! Defines argument and result structures for the `NLMPROC4_CANCEL`
//! operation as specified in RFC 1813.

use nfs_mamont_derive::XDRSize;

use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::Nlm4Stats;

/// Defines the information needed to cancel an outstanding lock request.
/// The data in the `Nlm4CancelArgs` structure must exactly match the corresponding information in the `Nlm4LockArgs` structure of the outstanding lock request to be cancelled.
pub struct Nlm4CancelArgs {
    /// Transaction identifier for matching request/response.
    pub cookie: Cookie,
    /// `True` if the client wishes the procedure call to block until the lock can be granted.
    /// A `false` value will cause the procedure call to return immediately if the lock cannot be granted.
    pub block: bool,
    /// `True` for exclusive lock, `false` for shared lock.
    pub exclusive: bool,
    /// Lock details (caller name, file handle, offset, length).
    pub lock: Nlm4Lock,
}

/// NLM CANCEL result.
///
/// Returned by [`NLMPROC4_CANCEL`](crate::consts::nlm::NLMPROC4_CANCEL) procedure.
#[derive(XDRSize)]
pub struct Nlm4CancelRes {
    /// Transaction identifier for matching request/response.
    pub cookie: Cookie,
    /// Status code (Granted, Denied, etc.).
    pub stat: Nlm4Stats,
}

/// Trait for handling NLMv4 `CANCEL` procedure calls.
///
/// Implementations should cancel a pending (blocked) lock request
/// that matches the given parameters.
#[trait_variant::make(Send)]
pub trait Cancel {
    async fn cancel(&self, args: Nlm4CancelArgs) -> Nlm4CancelRes;
}
