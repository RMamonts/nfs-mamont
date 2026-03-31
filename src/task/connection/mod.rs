//! Connection-specific tasks.
//!
//! This module provides the tasks for handling individual NFS client connections.
//! It implements a three-stage pipeline for processing RPC commands:
//!
//! - [`read::ReadTask`] - Reads RPC commands from the network connection
//! - [`vfs::VfsTask`] - Processes commands and performs VFS operations
//! - [`write::WriteTask`] - Writes operation results back to the network connection
//!
//! These tasks communicate via bounded channels to form an asynchronous processing pipeline
//! with built-in backpressure.

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::error;

use crate::context::ServerContext;
use crate::parser::NfsArgWrapper;
use crate::task::global::mount::MountCommand;
use crate::task::ProcReply;
use crate::vfs::Vfs;

mod read;
mod vfs;
mod write;

const COMMAND_CHANNEL_CAPACITY: usize = 512;
const RESULT_CHANNEL_CAPACITY: usize = 512;

// Creates all connection tasks with their inner connections
pub async fn new<V>(
    socket: TcpStream,
    mount_sender: mpsc::UnboundedSender<MountCommand>,
    context: &ServerContext<V>,
) where
    V: Vfs + Send + Sync + 'static,
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
    let (result_sender, result_receiver) = mpsc::channel::<ProcReply>(RESULT_CHANNEL_CAPACITY);
    // channel for request
    let (command_sender, command_receiver) =
        mpsc::channel::<NfsArgWrapper>(COMMAND_CHANNEL_CAPACITY);

    read::ReadTask::new(
        readhalf,
        peer_addr,
        command_sender,
        mount_sender,
        result_sender.clone(),
        context.get_write_allocator(),
    )
    .spawn();

    vfs::VfsTask::new(context, command_receiver, result_sender).spawn();

    write::WriteTask::new(writehalf, result_receiver).spawn();
}
