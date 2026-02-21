//! Defines NSM SM_UNMON_ALL [`UnmonitorAll`] interface (Procedure 4).
//!
//! As defined in XNFS, Version 3W (Open Group Technical Standard)
//! <https://pubs.opengroup.org/onlinepubs/9629799/SM_UNMON_ALL.htm>.

use async_trait::async_trait;

use super::{HostState, WatcherId};

/// Success result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Success {
    /// State number of the local NSM host.
    pub nsm_state: HostState,
}

/// Fail result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fail {
    /// State number of the local NSM host.
    pub host_state: HostState,
}

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`UnmonitorAll::unmonitor_all`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

#[async_trait]
pub trait UnmonitorAll {
    /// Stops monitoring all hosts for which monitoring was requested by the specified watcher.
    ///
    /// # Parameters:
    /// * `watcher_id` --- identifier of watcher. It must exactly match the information
    /// given in the corresponding `monitor` call.
    async fn unmonitor_all(&self, watcher_id: WatcherId, promise: impl Promise);
}
