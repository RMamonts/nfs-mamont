//! NLMv4 GRANTED procedure types.
//!
//! Defines argument and result structures for the `NLMPROC4_GRANTED`
//! operation as specified in RFC 1813.

use crate::nlm::cookie::Cookie;
use crate::nlm::Nlm4Stats;

/// NLM GRANTED result.
///
/// Returned by [`NLMPROC4_GRANTED`](crate::consts::nlm::NLMPROC4_GRANTED) procedure.
pub struct Nlm4GrantedRes {
    /// Transaction identifier for matching request/response.
    pub cookie: Cookie,
    /// Status code (Granted, Denied, etc.).
    pub stat: Nlm4Stats,
}

/// Trait for handling NLMv4 `GRANTED` procedure reply.
///
/// The implementation must process the result sent by the client in response to a call to the procedure of the same name.
#[trait_variant::make(Send)]
pub trait Granted {
    /// Coordinates the lock table with clients.
    ///
    /// ### Parameters
    /// * `res` — result arguments according to the RFC standard.
    async fn granted(&self, res: Nlm4GrantedRes);
}
