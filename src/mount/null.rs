//! Defines Mount version 3 [`Null`] interface (Procedure 0).
//!
//! as defined in RFC 1813 section 5.2.0.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.0>.

use async_trait::async_trait;

pub type Result = std::result::Result<(), ()>;

/// Defines callback to pass [`Null::null`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(result: Result);
}

#[async_trait]
pub trait Null {
    /// Does not do any work. It is made available to allow server response
    /// testing and timing.
    ///
    /// The procedure takes no MOUNT protocol arguments and returns no MOUNT protocol response.
    /// By convention, the procedure should never require any authentication.
    async fn null(&self, promise: impl Promise);
}
