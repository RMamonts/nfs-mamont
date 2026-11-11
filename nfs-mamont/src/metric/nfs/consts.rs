use std::time::Duration;

pub(in crate::metric) const STAGE_SECONDS: &str = "nfsmamont_rpc_stage_seconds";
pub(in crate::metric) const DURATION_SECONDS: &str = "nfsmamont_rpc_duration_seconds";
pub(in crate::metric) const REQUESTS_TOTAL: &str = "nfsmamont_rpc_requests_total";
pub(in crate::metric) const VFS_IN_FLIGHT: &str = "nfsmamont_vfs_in_flight";
pub(in crate::metric) const CPU_SECONDS: &str = "nfsmamont_rpc_cpu_seconds";
pub(in crate::metric) const SYSCALL_SECONDS: &str = "nfsmamont_rpc_syscall_seconds";
pub(in crate::metric) const WAIT_SECONDS: &str = "nfsmamont_rpc_wait_seconds";

/// How often tokio task metrics are sampled into the recorder.
pub(super) const TOKIO_SAMPLING_INTERVAL: Duration = Duration::from_secs(1);

/// `proc` label values, indexed by NFSv3 procedure number.
pub(super) const PROC_NAMES: [&str; 22] = [
    "null",
    "getattr",
    "setattr",
    "lookup",
    "access",
    "readlink",
    "read",
    "write",
    "create",
    "mkdir",
    "symlink",
    "mknod",
    "remove",
    "rmdir",
    "rename",
    "link",
    "readdir",
    "readdirplus",
    "fsstat",
    "fsinfo",
    "pathconf",
    "commit",
];
