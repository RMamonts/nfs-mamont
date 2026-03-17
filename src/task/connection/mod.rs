//! Connection-specific tasks.
//!
//! This module provides the tasks for handling individual NFS client connections.
//! It implements a three-stage pipeline for processing RPC commands:
//!
//! - [`read::ReadTask`] - Reads RPC commands from the network connection
//! - [`vfs::VfsTask`] - Processes commands and performs VFS operations
//! - [`write::WriteTask`] - Writes operation results back to the network connection
//!
//! These tasks communicate via unbounded channels to form an asynchronous processing pipeline.

use crate::context::ServerContext;
use crate::parser::NfsArgWrapper;
use crate::task::global::mount::MountCommand;
use crate::task::ProcReply;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

pub mod read;
pub mod vfs;
pub mod write;

// Creates all connection tasks with their inner connections
pub async fn new(
    socket: TcpStream,
    mount_sender: mpsc::UnboundedSender<MountCommand>,
    context: &ServerContext,
) {
    let peer_addr = match socket.peer_addr() {
        Ok(addr) => addr,
        Err(err) => {
            eprintln!("failed to determine peer address: {err}");
            return;
        }
    };
    let (readhalf, writehalf) = socket.into_split();
    // channel for result
    let (result_sender, result_receiver) = mpsc::unbounded_channel::<ProcReply>();
    // channel for request
    let (command_sender, command_receiver) = mpsc::unbounded_channel::<NfsArgWrapper>();

    read::ReadTask::new(
        readhalf,
        peer_addr,
        command_sender,
        mount_sender,
        result_sender.clone(),
        context.get_allocator(),
    )
    .spawn();

    vfs::VfsTask::new(context, command_receiver, result_sender).spawn();

    write::WriteTask::new(writehalf, result_receiver).spawn();
}
