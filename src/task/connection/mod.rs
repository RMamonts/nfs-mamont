//! Connection-specific tasks.
//!
//! This module provides the tasks for handling individual NFS client connections.
//! It implements a three-stage pipeline for processing RPC commands:
//!
//! - [`read::ReadTask`] - Reads RPC commands from the network connection
//! - [`write::WriteTask`] - Writes operation results back to the network connection
//!
//! These tasks communicate via unbounded channels to form an asynchronous processing pipeline.

use std::io;
use std::net::{SocketAddr, TcpStream as StdTcpStream};
use std::os::fd::{AsRawFd, FromRawFd};

use async_channel::Sender;
use tracing::error;
use tokio_uring::net::TcpStream;

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
    let socket = match duplicate_into_tokio_stream(socket) {
        Ok(socket) => socket,
        Err(err) => {
            error!(error=%err, "failed to adapt tokio-uring socket into tokio stream");
            return;
        }
    };
    let (readhalf, writehalf) = socket.into_split();
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

fn duplicate_into_tokio_stream(socket: TcpStream) -> io::Result<tokio::net::TcpStream> {
    let fd = socket.as_raw_fd();
    let dup_fd = unsafe { libc::dup(fd) };
    if dup_fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let std_stream = unsafe { StdTcpStream::from_raw_fd(dup_fd) };
    std_stream.set_nonblocking(true)?;
    tokio::net::TcpStream::from_std(std_stream)
}
