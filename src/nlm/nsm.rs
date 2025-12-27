use async_trait::async_trait;

/// Length of the private data.
pub const PRIVATE_LEN: usize = 16;

/// Name of the host.
/// Used for both `mon_name` (remote) and `my_name` (local).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostName(pub String);

/// NSM State Counter, that should be incremented on reboot.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostState(pub u32);

/// RPC Identity of the local process (e.g., NLM) asking for monitoring.
/// Corresponds to XDR `my_id`
#[allow(dead_code)]
#[derive(Clone)]
pub struct HostId {
    /// Name of the host where the callback process runs.
    pub name: HostName,
    /// RPC Program number.
    pub program: u32,
    /// RPC Version number.
    pub version: u32,
    /// RPC Procedure number to call back.
    pub proc: u32,
}

/// Monitor key: "monitor mon_name for my_id".
#[derive(Clone)]
pub struct MonitorKey {
    /// The host to watch (Remote server).
    pub name: HostName,
    /// The identity of the watcher (Usually local NLM)
    pub id: HostId
}

/// Arguments for monitor procedure. Corresponds to XDR `mon`.
#[allow(dead_code)]
#[derive(Clone)]
pub struct MonitorArgs {
    /// Monitor key: name of the host to monitor and watcher id.
    pub monitor_key: MonitorKey,
    /// Opaque data (cookie) returned in notify procedure.
    pub private: [u8; PRIVATE_LEN]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NsmFailError {
    /// Actual state number of the NSM the request was sent to
    pub state: HostState, 
}

/// Corresponds to XDR `sm_stat_res`.
/// Result for `monitor` and `stat`.
pub type NsmStatusResult = std::result::Result<HostState, NsmFailError>;

/// Corresponds to XDR `status`
/// Arguments for `notify`.
#[derive(Clone)]
pub struct NotifyArgs {
    /// Name of the host that rebooted (XDR: `mon_name`).
    pub host_name: HostName,
    /// The new state number of that host.
    pub state: HostState,
    /// The opaque cookie provided in MonitorArgs.
    pub private: [u8; PRIVATE_LEN]
}

mod Nsm {
    
}

#[async_trait]
pub trait Nsm: Sync + Send {
    /// This procedure does no work.
    /// It is made available to allow server response testing and timing.
    /// 
    /// Note: Corresponds to `SM_NULL` from NSM protocol.
    async fn null(&self);

    /// Registers a request to monitor a host.
    /// 
    /// Note: Corresponds to `SM_MON` from NSM protocol.
    async fn monitor(&self, args: MonitorArgs, promise: impl promise::Monitor);

    /// Stops monitoring a specific host for a specific client.
    /// Returns current local NSM state.
    /// 
    /// Note: Corresponds to `SM_UNMON` from NSM protocol.
    async fn unmonitor(&self, args: MonitorKey, promise: impl promise::State);

    /// Removes all monitoring requests from a specific client ID.
    /// Returns current local NSM state.
    /// 
    /// Note: Corresponds to `SM_UNMON_ALL` from NSM protocol.
    async fn unmonitor_all(&self, args: HostId, promise: impl promise::State);

    /// Notifies each host on its notify list of the change in state.
    /// The host will be found in the notify list if `monitor` (XDR: `SM_MON`)
    /// call was made to the NSM to register the host.
    /// 
    /// Note: Corresponds to `SM_NOTIFY` from NSM protocol.
    async fn notify(&self, args: NotifyArgs, promise: impl promise::Void);

    /// Debug helper to simulate a crash/reboot logic.
    /// 
    /// Note: Corresponds to `SM_SIMU_CRASH` from NSM protocol.
    async fn simulate_crash(&self, promise: impl promise::Void);

    /// Returns the status of the NSM itself.
    /// 
    /// Note: Corresponds to `SM_STAT` from NSM protocol.
    async fn stat(&self, promise: impl promise::Monitor);
}

mod promise {
    use crate::nlm::nsm::{HostState, NsmStatusResult};

    /// Promise to return the result of `monitor` and `stat` procedures.
    pub trait Monitor {
        fn keep(self, result: NsmStatusResult);
    }

    /// Promise to return the result of `unmonitor` and `unmonitor_all` procedures.
    /// The return value is the state number of the NSM server that processed the request.
    pub trait State {
        fn keep(self, result: HostState);
    }

    /// Promise to return make some post-processing actions after processing
    /// `notify` and `simulate_crash` procedures that have `void` return type.
    pub trait Void {
        fn keep(self);
    }
}
