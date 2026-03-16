//! Task management for NFS server operations.
//!
//! This module provides the task infrastructure for handling NFS server operations,
//! including connection-specific tasks and global task coordination.

use crate::mount::MountRes;
use crate::rpc::Error;
use crate::vfs::NfsRes;

pub mod connection;
pub mod global;

/// Tagged union of top-level RPC program results supported by this server.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub enum ProcResult {
    Nfs3(Box<NfsRes>),
    Mount(Box<MountRes>),
}

/// RPC reply metadata plus a typed result to be serialized.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct ProcReply {
    pub xid: u32,
    pub proc_result: Result<ProcResult, Error>,
}
