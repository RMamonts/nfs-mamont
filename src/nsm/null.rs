//! Defines NSM [`SM_NULL`] interface (Procedure 0).
//!
//! As defined in XNFS, Version 3W (Open Group Technical Standard)
//! <https://pubs.opengroup.org/onlinepubs/9629799/SM_SIMU_CRASH.htm>.

use async_trait::async_trait;

pub type Result = std::result::Result<(), ()>;

/// Defines callback to pass [`Null::null`] result into.
#[async_trait]
pub trait Promise {
    fn keep(promise: Result);
}

#[async_trait]
pub trait Null {
    /// Does nothing.
    ///
    /// It is made available to allow server response testing and timing.
    async fn null(&self, promise: impl Promise);
}
