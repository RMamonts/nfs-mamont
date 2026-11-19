use crate::allocator::Buffer;
use crate::connection_id::ConnectionId;
use crate::limits::budget::ReplySender;
use crate::vfs::nfsv4::compound;

/// Command sent to the NFSv4 task by the task manager (not yet handled).
#[allow(unused)]
pub struct Nfsv4Command<B: Buffer> {
    /// Channel used to pass the result to write task.
    pub result_tx: ReplySender<B>,
    /// The ID using the socket address.
    pub connection_id: ConnectionId,
    /// NFSv4 COMPOUND procedure arguments.
    pub args: Nfs4Arguments<B>,
}

/// Enumerates supported NFSv4 protocol procedure arguments.
#[allow(unused)]
pub enum Nfs4Arguments<B: Buffer> {
    /// Null operation arguments.
    Null,
    /// Compound operation arguments.
    Compound(compound::Args<B>),
}
