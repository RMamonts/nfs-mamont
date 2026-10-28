//! Connection-specific tasks.
//!
//! This module provides the tasks for handling individual NFS client connections.
//! It implements a two-stage pipeline for processing RPC commands:
//!
//! - [`read::ReadTask`] - Reads RPC commands from the network connection
//! - [`write::WriteTask`] - Writes operation results back to the network connection
//!
//! # In-flight limit
//!
//! Every connection has a budget of [`ServerContext::max_in_flight`] requests that are
//! read from the socket but whose replies are not yet written back. The read task takes
//! one [`limit::Token`] of the budget before it reads a request; the token travels with
//! the request through the global MOUNT/NLM/VFS tasks, comes back to the write task
//! inside an [`InFlightReply`] and is returned once the reply bytes are handed to the
//! socket. When the budget is exhausted the read task leaves the socket unread and TCP
//! flow control holds the client back. A record that gets no reply (a REPLY sent to the
//! server) gives its token back right away; every other record yields exactly one
//! [`InFlightReply`], see [`limit`] for what happens to a token that is dropped instead.
//!
//! The channels are unbounded, which is safe because of the budget: the result channel
//! of a connection never holds more than `max_in_flight` replies, so the shared
//! VFS/MOUNT/NLM tasks never block on a slow client, and the global command channels
//! together hold at most `max_in_flight × open connections` requests. Records the
//! server would initiate on its own (none today) are not covered by the budget.

use tokio::net::TcpStream;
use tracing::error;

use crate::allocator::{Allocator, Buffer};
use crate::context::ServerContext;
use crate::task::connection::limit::Budget;
use crate::task::connection::read::CommandSenders;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::nlm_task::NlmCommand;
use crate::task::InFlightReply;
use crate::vfs::Vfs;

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
    // channel for result
    let (result_sender, result_receiver) = async_channel::unbounded::<InFlightReply<B>>();
    // in-flight budget of the connection, see the module docs
    let (budget, limiter) = Budget::new(context.max_in_flight());

    let command_senders = CommandSenders::<B>::new(
        mount_sender,
        nlm_sender,
        context.get_vfs_manager().sender(),
        result_sender,
    );

    read::ReadTask::<A, B>::new(
        readhalf,
        peer_addr,
        context.get_allocator(),
        command_senders,
        budget,
    )
    .spawn();

    write::WriteTask::<B>::new(writehalf, result_receiver, limiter).spawn();
}
