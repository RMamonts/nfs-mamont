//! `NSM` protocol description as specified in XNFS, Version 3W (Open Group Technical Standard).
//! <https://pubs.opengroup.org/onlinepubs/9629799/chap11.htm>.

pub mod monitor;
pub mod notify;
pub mod null;
pub mod simulate_crash;
pub mod stat;
pub mod unmonitor;
pub mod unmonitor_all;

/// Length of the private data.
pub const PRIVATE_LEN: usize = 16;

#[allow(dead_code)]
#[derive(Clone)]
pub struct Cookie(pub [u8; PRIVATE_LEN]);

/// Name of the host to be monitored by the NSM.
/// Corresponds to XDR `sm_name`
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostName(pub String);

/// State counter of the host monitored by NSM or the NSM host itself,
/// that should be incremented on reboot.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostState(pub u32);

/// RPC identity of the local process (e.g., NLM) asking for monitoring.
/// Corresponds to XDR `my_id`
#[allow(dead_code)]
#[derive(Clone)]
pub struct WatcherId {
    /// Name of the host where the callback process runs.
    pub name: HostName,
    /// RPC Program number.
    pub program: u32,
    /// RPC Version number.
    pub version: u32,
    /// RPC Procedure number to call back.
    pub proc: u32,
}

/// Contains the name of the host to be monitored and the watcher's RPC call-back information.
#[allow(dead_code)]
#[derive(Clone)]
pub struct MonitorPair {
    /// The host to watch.
    pub name: HostName,
    /// The identity of the watcher.
    pub id: WatcherId,
}
