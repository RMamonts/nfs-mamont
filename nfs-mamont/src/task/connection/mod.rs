//! Connection-specific tasks.
//!
//! This module provides the tasks for handling individual NFS client connections.
//! It implements a three-stage pipeline for processing RPC commands:
//!
//! - [`read::ReadTask`] - Reads RPC commands from the network connection
//! - [`write::WriteTask`] - Writes operation results back to the network connection
//!
//! These tasks communicate via unbounded channels to form an asynchronous processing pipeline.

use std::net::SocketAddr;
use std::sync::Arc;

use async_channel::Sender;
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::allocator::{Allocator, Buffer};
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::NlmCommand;
use crate::task::global::vfs::VfsCommandSender;
use crate::task::ProcReply;

mod read;
mod write;

/// Resources of one server run shared by all of its connections.
pub struct ConnectionContext<A: Allocator> {
    /// Allocator serving `WRITE` payload buffers.
    pub allocator: Arc<A>,
    /// Queue of the task executing NFS procedures.
    pub vfs_sender: VfsCommandSender<A::Buffer>,
    /// Queue of the task executing MOUNT procedures.
    pub mount_sender: Sender<MountCommand<A::Buffer>>,
    /// Queue of the task executing NLM procedures.
    pub nlm_sender: Sender<NlmCommand<A::Buffer>>,
    /// Cancelled when the server stops: read tasks stop taking new requests.
    pub shutdown: CancellationToken,
}

/// Spawns the read and write tasks serving `socket` into `tasks`.
///
/// # Panics
///
/// If called outside of tokio runtime context.
pub fn spawn<A, B>(
    socket: TcpStream,
    client_addr: SocketAddr,
    context: &ConnectionContext<A>,
    tasks: &mut JoinSet<()>,
) where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
{
    let (readhalf, writehalf) = socket.into_split();
    // channel for result
    let (result_sender, result_receiver) = async_channel::unbounded::<ProcReply<B>>();

    read::ReadTask::<A, B>::new(readhalf, client_addr, result_sender, context).spawn(tasks);
    write::WriteTask::<B>::new(writehalf, result_receiver).spawn(tasks);
}
