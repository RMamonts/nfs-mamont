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
use tokio::sync::mpsc;
use tracing::error;

use crate::allocator::Allocator;
use crate::context::ServerContext;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::NlmCommand;
use crate::task::ProcReply;
use crate::vfs::Vfs;

mod read;
mod write;

// Creates all connection tasks with their inner connections
pub async fn new<A, V>(
    socket: TcpStream,
    mount_sender: mpsc::UnboundedSender<MountCommand>,
    nlm_sender: mpsc::UnboundedSender<NlmCommand>,
    context: &ServerContext<A, V>,
) where
    A: Allocator + Send + Sync + 'static,
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
    let (result_sender, result_receiver) = mpsc::unbounded_channel::<ProcReply>();
    // channel for request

    read::ReadTask::new(
        readhalf,
        peer_addr,
        mount_sender,
        nlm_sender,
        result_sender.clone(),
        context.get_write_allocator(),
        context.get_vfs_pool().sender(),
    )
    .spawn();

    write::WriteTask::new(writehalf, result_receiver).spawn();
}
