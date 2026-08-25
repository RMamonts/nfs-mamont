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
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::NlmCommand;
use crate::task::ProcReply;
use crate::vfs::Vfs;

mod read;
mod write;

// Creates all connection tasks with their inner connections
pub async fn new<AR, AW, V, BR, BW>(
    socket: TcpStream,
    mount_sender: async_channel::Sender<MountCommand<BR>>,
    nlm_sender: async_channel::Sender<NlmCommand<BR>>,
    context: &ServerContext<AR, AW, V, BR, BW>,
) where
    AR: Allocator<Buffer = BR> + Send + Sync + 'static,
    BR: Buffer + 'static,
    AW: Allocator<Buffer = BW> + Send + Sync + 'static,
    BW: Buffer + 'static,
    V: Vfs<BR, BW> + Send + Sync + 'static,
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
    let (result_sender, result_receiver) = async_channel::unbounded::<ProcReply<BR>>();
    // channel for request

    read::ReadTask::<AW, BR, BW>::new(
        readhalf,
        peer_addr,
        mount_sender,
        nlm_sender,
        result_sender.clone(),
        context.get_write_allocator(),
        context.get_vfs_pool().sender(),
    )
    .spawn();

    write::WriteTask::<BR>::new(writehalf, result_receiver).spawn();
}
