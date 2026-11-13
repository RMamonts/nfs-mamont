//! Connection-specific tasks.
//!
//! This module provides the tasks for handling individual NFS client connections.
//! It implements a three-stage pipeline for processing RPC commands:
//!
//! - [`read::ReadTask`] - Reads RPC commands from the network connection
//! - [`write::WriteTask`] - Writes operation results back to the network connection
//!
//! These tasks communicate via unbounded channels to form an asynchronous processing pipeline.
//! The read task holds the [`ConnectionLimits`] of the connection, which bound what the
//! channels hold, see [`crate::limits`].

use tokio::net::TcpStream;
use tokio::sync::OwnedSemaphorePermit;
use tracing::error;

use crate::allocator::{Allocator, Buffer};
use crate::connection_id::ConnectionId;
use crate::context::ServerContext;
use crate::limits::admission::Pass;
use crate::limits::budget::InFlightReply;
use crate::limits::connection::ConnectionLimits;
use crate::task::connection::read::CommandSenders;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::nlm_task::NlmCommand;
use crate::task::global::vfs::VfsSender;
use crate::vfs::Vfs;

mod read;
#[cfg(test)]
mod tests;
mod write;

// Creates all connection tasks with their inner connections
pub fn new<A, V, B>(
    socket: TcpStream,
    mount_sender: async_channel::Sender<MountCommand<B>>,
    nlm_sender: async_channel::Sender<NlmCommand<B>>,
    vfs_sender: VfsSender<B>,
    context: &ServerContext<A, V, B>,
    pass: Pass,
) where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: Vfs<B> + Send + Sync + 'static,
{
    let _ = socket.set_nodelay(true);
    let peer_addr = match socket.peer_addr() {
        Ok(addr) => addr,
        Err(err) => {
            error!(error=%err, "failed to determine peer address");
            return;
        }
    };
    let (readhalf, writehalf) = socket.into_split();
    let client_id = ConnectionId::new(peer_addr);
    // channel for result
    let (result_sender, result_receiver) = async_channel::unbounded::<InFlightReply<B>>();
    let limits = context.limits();
    let connection_limits = ConnectionLimits::new(limits, result_sender, Some(pass.ticket));

    let command_senders = CommandSenders::<B>::new(mount_sender, nlm_sender, vfs_sender);

    spawn_tasks(
        read::ReadTask::<A, B>::new(
            readhalf,
            client_id,
            context.get_allocator(),
            command_senders,
            connection_limits,
        ),
        write::WriteTask::<B>::new(writehalf, result_receiver, client_id, limits.write_timeout),
        pass.slot,
    );
}

/// Spawns both tasks of a connection.
///
/// The read task waits for slots that only the write task frees, so it is stopped once
/// the write task is gone, whether it finished, timed out or panicked. On a normal exit
/// it is already gone: it holds a sender of the result channel, and the write task stops
/// only when the channel is closed.
///
/// `slot`, the slot of the connection in [`crate::Limits::max_connections`], is given
/// back once the write task is gone.
fn spawn_tasks<A, B>(
    read_task: read::ReadTask<A, B>,
    write_task: write::WriteTask<B>,
    slot: Option<OwnedSemaphorePermit>,
) where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
{
    let read_task = read_task.spawn();
    let write_task = write_task.spawn();
    tokio::spawn(async move {
        let _ = write_task.await;
        read_task.abort();
        drop(slot);
    });
}
