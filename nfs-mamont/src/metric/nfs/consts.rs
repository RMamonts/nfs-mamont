use std::time::Duration;

pub const STAGE_SECONDS: &str = "nfsmamont_rpc_stage_seconds";
pub const DURATION_SECONDS: &str = "nfsmamont_rpc_duration_seconds";
pub const REQUESTS_TOTAL: &str = "nfsmamont_rpc_requests_total";
pub const VFS_IN_FLIGHT: &str = "nfsmamont_vfs_in_flight";
pub const CPU_SECONDS: &str = "nfsmamont_rpc_cpu_seconds";
pub const SYSCALL_SECONDS: &str = "nfsmamont_rpc_syscall_seconds";
pub const WAIT_SECONDS: &str = "nfsmamont_rpc_wait_seconds";
pub const SAMPLE_EVERY_GAUGE: &str = "nfsmamont_sample_every";

/// About one in this many NFS requests is timed.
pub const SAMPLE_EVERY: u64 = 16;

/// How often tokio task metrics are sampled into the recorder.
pub const TOKIO_SAMPLING_INTERVAL: Duration = Duration::from_secs(1);

/// `proc` label values, indexed by NFSv3 procedure number.
pub const PROC_NAMES: [&str; 22] = [
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
