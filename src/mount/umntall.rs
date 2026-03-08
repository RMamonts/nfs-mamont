//! Defines Mount version 3 [`Umntall`] interface (Procedure 4).
//!
//! as defined in RFC 1813 section 5.2.4.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.4>.

use async_trait::async_trait;

/// Defines callback to pass [`Umntall::umntall`] result into.
#[async_trait]
pub trait Promise {
    /// Persists the result of the UMNTALL procedure.
    async fn keep();
}

/// Mount version 3 UMNTALL procedure.
#[async_trait]
pub trait Umntall {
    /// Removes all of the mount entries for this client previously
    /// recorded by calls to MNT.
    async fn umntall(&self);
}
