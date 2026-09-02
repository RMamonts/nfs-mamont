//! NLMv4 UNLOCK procedure types.
//!
//! Defines argument and result structures for the `NLMPROC4_UNLOCK`
//! operation as specified in RFC 1813.

use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::Nlm4Stats;

/// Defines the information needed to remove a previously established lock.
pub struct Nlm4UnlockArgs {
    /// Transaction identifier for matching request/response.
    pub cookie: Cookie,
    /// Lock details (caller name, file handle, offset, length).
    pub lock: Nlm4Lock,
}

/// NLM UNLOCK result.
///
/// Returned by [`NLMPROC4_UNLOCK`](crate::consts::nlm::NLMPROC4_UNLOCK) procedure.
pub struct Nlm4UnlockRes {
    /// Transaction identifier for matching request/response.
    pub cookie: Cookie,
    /// Status code (Granted, Denied, etc.).
    pub stat: Nlm4Stats,
}

/// Trait for handling NLMv4 `UNLOCK` procedure calls.
///
/// Implementations should remove a previously granted lock matching
/// the request parameters and return the result status.
#[trait_variant::make(Send)]
pub trait Unlock {
    async fn unlock(&self, args: Nlm4UnlockArgs) -> Nlm4UnlockRes;
}
