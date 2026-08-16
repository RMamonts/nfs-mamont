//! Connection-specific tasks.
//!
//! This module provides the tasks for handling individual NFS client connections.
//! It implements a three-stage pipeline for processing RPC commands:
//!
//! - [`read::ReadTask`] - Reads RPC commands from the network connection
//! - [`write::WriteTask`] - Writes operation results back to the network connection
//!
//! These tasks communicate via unbounded channels to form an asynchronous processing pipeline.

use tokio::net::TcpStream;
use tracing::error;

use crate::allocator::{Allocator, Buffer};
use crate::context::ServerContext;
use crate::nlm::cookie::Cookie;
use crate::task::connection::read::CommandSenders;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::NlmCommand;
use crate::task::ProcReply;
use crate::vfs::Vfs;

mod read;
mod write;

// Creates all connection tasks with their inner connections
pub async fn new<A, V, B>(
    socket: TcpStream,
    mount_sender: async_channel::Sender<MountCommand<B>>,
    nlm_sender: async_channel::Sender<NlmCommand<B>>,
    context: &ServerContext<A, V, B>,
) where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: Vfs<B> + Send + Sync + 'static,
{
    let peer_addr = match socket.peer_addr() {
        Ok(addr) => addr,
        Err(err) => {
            error!(error=%err, "failed to determine peer address");
            return;
        }
    };
    let (readhalf, writehalf) = socket.into_split();
    // channel for result
    let (result_sender, result_receiver) = async_channel::unbounded::<ProcReply<B>>();
    // channel for request

    let (granted_tx, granted_rx) = async_channel::unbounded::<Cookie>();

    let main_senders = CommandSenders::<B>::new(
        mount_sender,
        nlm_sender,
        result_sender.clone(),
        context.get_allocator(),
        context.get_vfs_pool().sender(),
        result_sender.clone(),
        granted_tx,
    );
    read::ReadTask::<A, B>::new(readhalf, peer_addr, context.get_write_allocator(), main_senders)
        .spawn();

    write::WriteTask::<B>::new(writehalf, result_receiver, granted_rx).spawn();
}
