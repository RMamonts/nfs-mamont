//! Connection-specific tasks.
//!
//! This module provides the tasks for handling individual NFS client connections.
//! It implements a three-stage pipeline for processing RPC commands:
//!
//! - [`read::ReadTask`] - Reads RPC commands from the network connection
//! - [`write::WriteTask`] - Writes operation results back to the network connection
//!
//! These tasks communicate via unbounded channels to form an asynchronous processing pipeline.
//!
//! # In-flight limit
//!
//! A connection may have [`ServerContext::max_in_flight`] requests read from the socket
//! but not yet answered. The read task takes a [`limit::ReplySender`] from its
//! [`limit::Budget`] before it reads a request; the slot is freed once the write task has
//! handed the reply to the socket, or when the request is dropped on its way. While no
//! slot is free the socket stays unread and TCP flow control holds the client back.
//!
//! This is what makes the unbounded channels safe: a result channel holds at most
//! `max_in_flight` replies, so the shared VFS/MOUNT/NLM tasks never block on a slow
//! client, and the global channels hold at most `max_in_flight × open connections`
//! requests.

use tokio::net::TcpStream;
use tracing::error;

use crate::allocator::{Allocator, Buffer};
use crate::connection_id::ConnectionId;
use crate::context::ServerContext;
use crate::task::connection::limit::{Budget, InFlightReply};
use crate::task::connection::read::CommandSenders;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::nlm_task::NlmCommand;
use crate::vfs::nfsv3::Vfs;

pub mod limit;
mod read;
#[cfg(test)]
mod tests;
mod write;

// Creates all connection tasks with their inner connections
pub fn new<A, V, B>(
    socket: TcpStream,
    mount_sender: async_channel::Sender<MountCommand<B>>,
    nlm_sender: async_channel::Sender<NlmCommand<B>>,
    context: &ServerContext<A, V, B>,
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
    // in-flight budget of the connection, see the module docs
    let budget =
        Budget::new(context.max_in_flight(), context.max_data_per_connection(), result_sender);

    let command_senders =
        CommandSenders::<B>::new(mount_sender, nlm_sender, context.get_vfs_manager().sender());

    spawn_tasks(
        read::ReadTask::<A, B>::new(
            readhalf,
            client_id,
            context.get_allocator(),
            command_senders,
            budget,
        ),
        write::WriteTask::<B>::new(writehalf, result_receiver, client_id, context.write_timeout()),
    );
}

/// Spawns both tasks of a connection.
///
/// The read task waits for slots that only the write task frees, so it is stopped once
/// the write task is gone, whether it finished, timed out or panicked. On a normal exit
/// it is already gone: it holds a sender of the result channel, and the write task stops
/// only when the channel is closed.
fn spawn_tasks<A, B>(read_task: read::ReadTask<A, B>, write_task: write::WriteTask<B>)
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
{
    let read_task = read_task.spawn();
    let write_task = write_task.spawn();
    tokio::spawn(async move {
        let _ = write_task.await;
        read_task.abort();
    });
}
