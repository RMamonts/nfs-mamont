//! Defines NSM [`SM_NOTIFY`] interface (Procedure 6).
//!
//! As defined in XNFS, Version 3W (Open Group Technical Standard)
//! <https://pubs.opengroup.org/onlinepubs/9629799/SM_NOTIFY.htm>.

use async_trait::async_trait;

use super::{Cookie, HostName, HostState};

/// Status message, that is sent to clients by the NSM, local to the host
/// that had the status change.
pub struct StatusMessage {
    /// Name of the host that had the state change (copied from parameters)
    pub host_name: HostName,
    /// The new state number of the rebooted host (copied from parameters)
    pub state: HostState,
    /// The opaque cookie provided by watcher in arguments to `monitor` call.
    pub cookie: Cookie,
}

/// Defines callback to pass [`Notify::notify`] result into.
#[async_trait]
pub trait Promise {
    fn keep();
}

#[async_trait]
pub trait Notify {
    /// Notifies each watcher from the notify list of some monitored host
    /// if the host state changed. The procedure should be called by the local NSM.
    ///
    /// # Parameters:
    ///
    /// * `host_name` --- Name of the host that had the state change (XDR: `mon_name`).
    /// * `state` --- The new state number of the host that had the state change.
    ///
    /// The status message that is sent to watchers by NSM host is described in [`StatusMessage`].
    ///
    /// NSM saves the name of the host to monitor in a notify list on stable storage.
    /// If the host running the NSM crashes, on reboot it must send out a notify call
    /// to each host in the notify list.
    async fn notify(&self, host_name: HostName, state: HostState, promise: impl Promise);
}
