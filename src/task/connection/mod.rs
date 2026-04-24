//! Connection-specific tasks.
//!
//! This module provides the tasks for handling individual NFS client connections.
//! It implements a three-stage pipeline for processing RPC commands:
//!
//! - [`read::ReadTask`] - Reads RPC commands from the network connection
//! - [`write::WriteTask`] - Writes operation results back to the network connection
//!
//! These tasks communicate via bounded channels to form an asynchronous processing pipeline
//! with built-in backpressure.

use tracing::error;

use crate::context::ServerContext;
use crate::runtime;
use crate::task::global::mount::MountCommand;
use crate::task::ProcReply;
use crate::vfs::Vfs;

mod read;
mod write;

pub async fn new<V>(
    socket: runtime::net::TcpStream,
    mount_sender: runtime::sync::mpsc::UnboundedSender<MountCommand>,
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
    let (result_sender, result_receiver) = runtime::sync::mpsc::unbounded_channel::<ProcReply>();

    read::ReadTask::new(
        readhalf,
        peer_addr,
        mount_sender,
        result_sender,
        context.get_write_allocator(),
        context.get_vfs_pool().sender(),
    )
    .spawn();

    write::WriteTask::new(writehalf, result_receiver).spawn();
}
