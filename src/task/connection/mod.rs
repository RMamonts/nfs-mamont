//! Connection-specific tasks.
//!
//! This module provides the tasks for handling individual NFS client connections.
//! It implements a three-stage pipeline for processing RPC commands:
//!
//! - [`read::ReadTask`] - Reads RPC commands from the network connection
//! - [`write::WriteTask`] - Writes operation results back to the network connection
//!
//! These tasks communicate via unbounded channels to form an asynchronous processing pipeline.

use async_channel::Sender;
use std::net::SocketAddr;
use std::rc::Rc;
use tokio_uring::net::TcpStream;

use crate::context::ServerContext;
use crate::rpc_io::{UringReadHalf, UringWriteHalf};
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
    let shared = Rc::new(socket);
    let readhalf = UringReadHalf::new(shared.clone());
    let writehalf = UringWriteHalf::new(shared);
    // channel for result
    let (result_sender, result_receiver) = async_channel::unbounded::<ProcReply>();
    // channel for request

    read::ReadTask::new(
        readhalf,
        peer_addr,
        mount_sender,
        result_sender.clone(),
        context.get_write_allocator(),
        context.get_vfs_pool().sender(),
    )
    .spawn();

    write::WriteTask::new(writehalf, result_receiver).spawn();
}
