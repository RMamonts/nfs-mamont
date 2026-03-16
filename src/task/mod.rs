//! Task management for NFS server operations.
//!
//! This module provides the task infrastructure for handling NFS server operations,
//! including connection-specific tasks and global task coordination.

use crate::interface::mount::MountRes;
use crate::interface::vfs::NfsRes;
use crate::rpc::Error;

pub mod connection;
pub mod global;

/// Tagged union of top-level RPC program results supported by this server.
pub enum ProcResult {
    Nfs3(Box<NfsRes>),
    Mount(Box<MountRes>),
}

/// RPC reply metadata plus a typed result to be serialized.
pub struct ProcReply {
    pub xid: u32,
    pub proc_result: Result<ProcResult, Error>,
}
