//! Task management for NFS server operations.
//!
//! This module provides the task infrastructure for handling NFS server operations,
//! including connection-specific tasks and global task coordination.

use crate::allocator::Buffer;
use crate::mount::MountRes;
use crate::nlm::NlmRes;
use crate::rpc::Error;
use crate::task::connection::limit::Token;
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

/// A reply on its way to the write task of the connection it belongs to.
///
/// `token` is the in-flight slot of the request being answered, see the
/// [`connection`] module for the limit. The write task frees it only after the reply
/// bytes are handed to the socket, so replies that cannot be written yet keep their
/// slots and the read task of the connection stops taking new requests.
pub struct InFlightReply<B: Buffer> {
    /// The reply to serialize.
    pub reply: ProcReply<B>,
    /// In-flight slot of the answered request.
    pub token: Token,
}
