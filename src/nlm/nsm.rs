use async_trait::async_trait;

/// Length of the private data.
pub const PRIVATE_LEN: usize = 16;

pub type StatusResult 

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// NSM agrees to monitor.
    Success = 0,
    /// NSM cannot monitor.
    Fail = 1
}

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
#[allow(dead_code)]
#[derive(Clone)]
pub struct MonitorKey {
    /// The host to watch (Remote server).
    pub name: HostName,
    /// The identity of the watcher (Usually local NLM)
    pub id: HostId
}

/// Corresponds to XDR `mon`.
/// Arguments for monitor procedure.
#[allow(dead_code)]
#[derive(Clone)]
pub struct MonitorArgs {
    /// Monitor key: name of the host to monitor and watcher id.
    pub monitor_key: MonitorKey,
    /// Opaque data (cookie) returned in notify procedure.
    pub private: [u8; PRIVATE_LEN]
}

/// Corresponds to XDR `sm_stat_res`.
/// Result for `monitor` and `stat`.
#[derive(Clone)]
pub struct NsmStatusResult {
    /// Return status of the call, that indicates,
    /// whether NSM agreed to monitor the given host or not.
    pub result: Status,
    /// State number of the NSM the request was sent to.
    pub state: HostState
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NsmFailError {
    /// Актуальный state number NSM-сервера, который обработал запрос.
    pub state: HostState, 
}

pub type NsmStatusResult = std::result::Result<HostState, Nsm>

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

#[async_trait]
pub trait Nsm: Sync + Send {
    /// This procedure does no work.
    /// It is made available to allow server response testing and timing.
    async fn null(&self);

    /// Registers a request to monitor a host.
    async fn monitor(&self, args: MonitorArgs, promise: impl promise::Monitor);

    /// Stops monitoring a specific host for a specific client.
    /// Returns current local NSM state.
    async fn unmonitor(&self, args: MonitorKey, promise: impl promise::State);

    /// Removes all monitoring requests from a specific client ID.
    /// Returns current local NSM state.
    async fn unmonitor_all(&self, args: HostId, promise: impl promise::State);

    async fn notify(&self, args: NotifyArgs, promise: impl promise::Void);

    /// Debug helper to simulate a crash/reboot logic.
    async fn simu_crash(&self, promise: impl promise::Void);

    /// Returns the status of the NSM itself.
    async fn stat(&self, promise: impl promise::Monitor);
}

mod promise {
    use crate::nlm::nsm::{HostState};

    use super::NsmStatusResult;

    /// Promise to return the result of `monitor` and `stat` procedures,
    /// i. e. 
    pub trait Monitor {
        fn keep(self, result: NsmStatusResult);
    }

    pub trait State {
        fn keep(self, result: HostState);
    }

    pub trait Void {
        fn keep(self);
    }
}