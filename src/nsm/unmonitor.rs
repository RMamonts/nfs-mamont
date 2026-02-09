//! Defines NSM SM_UNMON [`Unmonitor`] interface (Procedure 3).
//!
//! As defined in XNFS, Version 3W (Open Group Technical Standard)
//! <https://pubs.opengroup.org/onlinepubs/9629799/SM_UNMON.htm>.

use async_trait::async_trait;

use super::{HostState, MonitorPair};

/// Success result.
pub struct Success {
    /// State number of the local NSM host.
    pub nsm_state: HostState,
}

/// Fail result.
pub struct Fail {
    /// State number of the local NSM host.
    pub host_state: HostState,
}

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Unmonitor::unmonitor`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

#[async_trait]
pub trait Unmonitor {
    /// Stops monitoring the host specified by `monitor_pair.name`.
    ///
    /// # Parameters:
    /// * `monitor_pair` --- Name of the host to monitor and watcher id.
    /// The information in `monitor_pair` must exactly match the information
    /// given in the corresponding `monitor` call.
    async fn unmonitor(&self, monitor_pair: MonitorPair, promise: impl Promise);
}
