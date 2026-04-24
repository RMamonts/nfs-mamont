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
use tokio_uring::net::TcpStream;
use tracing::error;

use crate::context::ServerContext;
use crate::task::global::mount::MountCommand;
use crate::task::ProcReply;

mod read;
mod write;

// Creates all connection tasks with their inner connections
pub async fn new(
    socket: TcpStream,
    peer_addr: SocketAddr,
    mount_sender: Sender<MountCommand>,
    context: &ServerContext,
) {
    let socket = Arc::new(socket);

    if let Err(err) = socket.set_nodelay(true) {
        error!(error=%err, "failed to set TCP_NODELAY");
    }

    // channel for result
    let (result_sender, result_receiver) = async_channel::unbounded::<ProcReply>();
    // channel for request

    read::ReadTask::new(
        Arc::clone(&socket),
        peer_addr,
        mount_sender,
        result_sender.clone(),
        context.get_write_allocator(),
        context.get_vfs_pool().sender(),
    )
    .spawn();

    write::WriteTask::new(socket, result_receiver).spawn();
}
