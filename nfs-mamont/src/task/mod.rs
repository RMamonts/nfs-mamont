//! Task management for NFS server operations.
//!
//! This module provides the task infrastructure for handling NFS server operations,
//! including connection-specific tasks and global task coordination.

use crate::allocator::Buffer;
use crate::mount::MountRes;
use crate::nlm::{NlmCallbackReply, NlmRes};
use crate::rpc::Error;
use crate::vfs::NfsRes;

pub mod connection;
pub mod global;

/// Tagged union of top-level RPC program results supported by this server.
pub enum ProcResult<B: Buffer> {
    Nfs3(Box<NfsRes<B>>),
    Mount(Box<MountRes>),
    Nlm4(Box<NlmRes>),
}

pub enum ProcMessage {
    Nlm4(NlmCallbackReply),
}

/// RPC reply metadata plus a typed result to be serialized.
pub struct ProcReply<B: Buffer> {
    pub xid: u32,
    pub proc_result: Result<ProcResult<B>, Error>,
}

/// RPC call metadata plus a typed message to be serialized.
pub struct ProcCall {
    pub proc_message: ProcMessage,
}

impl ProcCall {
    pub fn new(proc_message: ProcMessage) -> ProcCall {
        ProcCall { proc_message }
    }
}
