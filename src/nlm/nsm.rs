use async_trait::async_trait;

pub const PRIVATE_LEN: usize = 32;

#[allow(dead_code)]
pub enum Result {
    Success = 0,
    Fail = 1
}

/// Name of the host to be monitored by the NSM.
#[allow(dead_code)]
pub struct HostName(pub String);

#[allow(dead_code)]
pub struct HostState(pub u32);

#[allow(dead_code)]
pub struct HostId {
    pub name: HostName,
    pub program: u32,
    pub version: u32,
    pub proc: u32,
}

#[allow(dead_code)]
pub struct MonitorId {
    // TODO: monitor name?
    pub name: HostName,
    pub id: HostId
}

#[allow(dead_code)]
pub struct Monitor {
    pub monitor_id: MonitorId,
    pub private: [u8; PRIVATE_LEN]
}

pub struct MonitorResult {
    pub result: Result,
    pub state: HostState
}

#[async_trait]
pub trait Nsm: Sync + Send {
    async fn monitor(&self, promise: impl promise::Monitor);
}

mod promise {
    use super::MonitorResult;

    pub trait Monitor {
        fn keep(self, result: MonitorResult);
    }
}