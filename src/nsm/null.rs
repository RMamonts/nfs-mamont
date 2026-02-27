//! Defines NSM SM_NULL [`Null`] interface (Procedure 0).
//!
//! As defined in XNFS, Version 3W (Open Group Technical Standard)
//! <https://pubs.opengroup.org/onlinepubs/9629799/SM_NULL.htm>.

use async_trait::async_trait;

/// Defines callback to pass [`Null::null`] result into.
#[async_trait]
pub trait Promise {
    async fn keep();
}

#[async_trait]
pub trait Null {
    /// Does nothing.
    ///
    /// It is made available to allow server response testing and timing.
    async fn null(&self, promise: impl Promise);
}
