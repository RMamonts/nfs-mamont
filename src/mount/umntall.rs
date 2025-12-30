//! Defines Mount version 4 [`Umntall`] interface (Procedure 4).
//!
//! as defined in RFC 1813 section 5.2.4.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.4>.
use async_trait::async_trait;

/// Defines callback to pass [`Umntall::umnt`] result into.
#[async_trait]
pub trait Promise {
    async fn keep();
}

#[async_trait]
pub trait Umntall {
    /// Removes all of the mount entries for this client previously.
    /// recorded by calls to MNT.
    ///
    /// AUTH_UNIX authentication or better is required.
    /// There are no MOUNT protocol errors which can be returned from this procedure.
    async fn umntall(&self, promise: impl Promise);
}
