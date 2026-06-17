//! Task management for NFS server operations.
//!
//! This module provides the task infrastructure for handling NFS server operations,
//! including connection-specific tasks and global task coordination.

use crate::allocator::Buffer;
use crate::mount::MountRes;
use crate::nlm::NlmRes;
use crate::rpc::Error;
use crate::vfs::NfsRes;

pub mod connection;
pub mod global;

/// Tagged union of top-level RPC program results supported by this server.
#[cfg_attr(
    feature = "arbitrary",
    derive(arbitrary::Arbitrary, Debug),
    arbitrary(bound = "B: for <'a> arbitrary::Arbitrary<'a> + Buffer")
)]
pub enum ProcResult<B: Buffer> {
    Nfs3(Box<NfsRes<B>>),
    Mount(Box<MountRes>),
    Nlm4(Box<NlmRes>),
}

/// RPC reply metadata plus a typed result to be serialized.
#[cfg_attr(
    feature = "arbitrary",
    derive(arbitrary::Arbitrary, Debug),
    arbitrary(bound = "B: for <'a> arbitrary::Arbitrary<'a> + Buffer")
)]
pub struct ProcReply<B: Buffer> {
    pub xid: u32,
    pub proc_result: Result<ProcResult<B>, Error>,
}
