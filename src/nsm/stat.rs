//! Defines NSM [`SM_STAT`] interface (Procedure 1).
//!
//! As defined in XNFS, Version 3W (Open Group Technical Standard)
//! <https://pubs.opengroup.org/onlinepubs/9629799/SM_STAT.htm>.

use async_trait::async_trait;

use super::{HostName, HostState};

/// Success result, meaning the NSM AGREED to monitor the specified host.
pub struct Success {
    /// State number of the local NSM host.
    pub nsm_state: HostState,
}

/// Fail result, meaning the NSM is NOT ABLE to monitor the specified host.
pub struct Fail {
    /// State number of the local NSM host.
    pub host_state: HostState,
}

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Stat::stat`] result into.
#[async_trait]
pub trait Promise {
    fn keep(promise: Result);
}

#[async_trait]
pub trait Stat {
    /// Tests to see whether the NSM agrees to monitor the given host.
    ///
    /// # Parameters:
    /// * `host_name` --- Name of the host to monitor.
    ///
    /// Note: implementations should not rely on this procedure being operative.
    /// In many current implementations of the NSM it will always return a `Fail` status.
    async fn stat(&self, host_name: HostName, promise: impl Promise);
}
