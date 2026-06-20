//! NLMv4 TEST procedure types.
//!
//! Defines argument and result structures for the `NLMPROC4_TEST`
//! operation as specified in RFC 1813.

use crate::nlm::cookie::Cookie;
use crate::nlm::holder::Nlm4Holder;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::Nlm4Stats;
use nfs_mamont_derive::XDRSize;

/// NLM TEST arguments.
///
/// Used to test whether a lock can be granted without actually acquiring it.
pub struct Nlm4TestArgs {
    /// Transaction identifier for matching request/response.
    pub cookie: Cookie,
    /// `True` for exclusive lock, `false` for shared lock.
    pub exclusive: bool,
    /// Lock details (caller name, file handle, offset, length).
    pub lock: Nlm4Lock,
}

/// NLM TEST result.
///
/// Returned by [`NLMPROC4_TEST`](crate::consts::nlm::NLMPROC4_TEST) procedure.
#[derive(XDRSize)]
pub struct Nlm4TestRes {
    /// Transaction identifier for matching request/response.
    pub cookie: Cookie,
    /// Status with optional holder info (if denied).
    pub test_stat: Nlm4TestReply,
}

/// NLM TEST reply status union.
///
/// Contains either granted status (no holder), or denied with holder info.
#[derive(XDRSize)]
pub struct Nlm4TestReply {
    /// Status code (Granted, Denied, etc.).
    pub stat: Nlm4Stats,
    /// Present only when stat is Denied — info about current lock holder.
    pub holder: Option<Nlm4Holder>,
}

/// Trait for handling NLMv4 `TEST` procedure calls.
///
/// Implementations should check whether a lock could be granted
/// without actually acquiring it, returning either `Granted` or
/// `Denied` with details of the conflicting lock.
#[trait_variant::make(Send)]
pub trait Test {
    async fn test(&self, args: Nlm4TestArgs) -> Nlm4TestRes;
}
