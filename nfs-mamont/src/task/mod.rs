//! Task management for NFS server operations.
//!
//! This module provides the task infrastructure for handling NFS server operations,
//! including connection-specific tasks and global task coordination.

use crate::allocator::Buffer;
use crate::mount::MountRes;
use crate::nlm::NlmRes;
use crate::rpc::{Error, OpaqueAuth};
use crate::vfs::NfsRes;

pub mod connection;
pub mod global;

/// Tagged union of top-level RPC program results supported by this server.
pub enum ProcResult<B: Buffer> {
    Nfs3(Box<NfsRes<B>>),
    Mount(Box<MountRes>),
    Nlm4(Box<NlmRes>),
}

/// RPC reply metadata plus a typed result to be serialized.
pub struct ProcReply<B: Buffer> {
    pub xid: u32,
    pub proc_result: Result<ProcResult<B>, Error>,
}

pub struct RPCReply<B: Buffer> {
    pub xid: u32,
    pub verifier: OpaqueAuth,
    pub result: Result<ProcResult<B>, Error>,
}
