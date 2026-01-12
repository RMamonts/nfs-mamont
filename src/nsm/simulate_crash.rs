//! Defines NSM SM_SIMU_CRASH [`SimulateCrash`] interface (Procedure 5).
//!
//! As defined in XNFS, Version 3W (Open Group Technical Standard)
//! <https://pubs.opengroup.org/onlinepubs/9629799/SM_SIMU_CRASH.htm>.

use async_trait::async_trait;

/// Defines callback to pass [`SimulateCrash::simulate_crash`] result into.
#[async_trait]
pub trait Promise {
    fn keep();
}

#[async_trait]
pub trait SimulateCrash {
    /// Simulates a crash of the NSM server.
    ///
    /// The NSM releases all its current state information and reinitialises itself,
    /// incrementing its state variable. It reads through its notify list (see `monitor`)
    /// and informs the NSM on all hosts on the list that the state of this host
    /// has changed, via the `notify` procedure.
    async fn simulate_crash(&self, promise: impl Promise);
}
