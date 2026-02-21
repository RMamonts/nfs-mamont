//! `NSM` protocol description as specified in XNFS, Version 3W (Open Group Technical Standard).
//! <https://pubs.opengroup.org/onlinepubs/9629799/chap11.htm>.

use async_trait::async_trait;

pub mod monitor;
pub mod notify;
pub mod null;
pub mod simulate_crash;
pub mod stat;
pub mod unmonitor;
pub mod unmonitor_all;

/// NSM program number.
pub const SM_PROG: u32 = 100024;

/// NSM protocol version.
pub const SM_VERS: u32 = 1;

/// Length of the private data.
pub const PRIVATE_LEN: usize = 16;

/// Opaque private data provided by the watcher in [`monitor::Monitor::monitor`]
/// and returned in [`notify::StatusMessage`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie(pub [u8; PRIVATE_LEN]);

/// Name of the host to be monitored by the NSM.
/// Corresponds to XDR `sm_name`

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostName(pub String);

/// State counter of the host monitored by NSM or the NSM host itself,
/// that should be incremented on reboot.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostState(pub u32);

/// RPC identity of the local process (e.g., NLM) asking for monitoring.
/// Corresponds to XDR `my_id`

#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorPair {
    /// The host to watch.
    pub name: HostName,
    /// The identity of the watcher.
    pub id: WatcherId,
}

/// NSM service trait.
#[async_trait]
pub trait Nsm:
    null::Null
    + stat::Stat
    + monitor::Monitor
    + unmonitor::Unmonitor
    + unmonitor_all::UnmonitorAll
    + simulate_crash::SimulateCrash
    + notify::Notify
{
}
