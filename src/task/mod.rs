//! Task management for NFS server operations.
//!
//! This module provides the task infrastructure for handling NFS server operations,
//! including connection-specific tasks and global task coordination.

use crate::mount::MountRes;
use crate::rpc::Error;
use crate::vfs::NfsRes;

pub(crate) mod connection;
pub(crate) mod global;

/// Tagged union of top-level RPC program results supported by this server.
pub(crate) enum ProcResult {
    Nfs3(Box<NfsRes>),
    Mount(Box<MountRes>),
}

/// RPC reply metadata plus a typed result to be serialized.
pub(crate) struct ProcReply {
    pub(crate) xid: u32,
    pub(crate) proc_result: Result<ProcResult, Error>,
}
