//! Defines NSM [`SM_MON`] interface (Procedure 2).
//!
//! As defined in XNFS, Version 3W (Open Group Technical Standard)
//! <https://pubs.opengroup.org/onlinepubs/9629799/SM_MON.htm>.

use async_trait::async_trait;

use super::{Cookie, HostState, MonitorPair};

/// Success result.
pub struct Success {
    /// State number of the remote NSM host.
    pub nsm_state: HostState,
}

/// Fail result.
pub struct Fail {
    /// State number of the remote NSM host.
    pub host_state: HostState,
}

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Monitor::monitor`] result into.
#[async_trait]
pub trait Promise {
    fn keep(promise: Result);
}

#[async_trait]
pub trait Monitor {
    /// Initiates the monitoring of the given host.
    ///
    /// # Parameters:
    ///
    /// * `monitor_pair` --- Name of the host to monitor and watcher id.
    /// * `cookie` --- Watcher private information, opaque to the server.
    /// The NSM server sends it in the notify call.
    ///
    /// NSM saves the name of the host to monitor in a notify list on stable storage.
    /// If the host running the NSM crashes, on reboot it must send out a notify call
    /// to each host in the notify list.
    async fn monitor(&self, monitor_pair: MonitorPair, cookie: Cookie, promise: impl Promise);
}
