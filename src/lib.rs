//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

pub mod allocator;
pub mod client;
pub mod mount;
pub mod nfsv3;
pub mod parser;
mod read_task;
pub mod rpc;
pub mod serializer;
pub mod vfs;
mod vfs_task;
mod write_task;

use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::read_task::ReadTask;
use crate::rpc::{CommandResult, ConnectionContext, RpcCommand, ServerContext};
use crate::vfs_task::VfsTask;
use crate::write_task::WriteTask;

/// Starts the NFS server and processes client connections.
pub async fn handle_forever(listener: TcpListener) -> std::io::Result<()> {
    handle_forever_with_context(listener, ServerContext::default()).await
}

/// Starts the NFS server and processes client connections using explicit server state.
pub async fn handle_forever_with_context(
    listener: TcpListener,
    server_context: ServerContext,
) -> std::io::Result<()> {
    loop {
        let (socket, _) = listener.accept().await?;
        eprintln!(
            "accepted connection local={} peer={}",
            socket
                .local_addr()
                .map(|addr| addr.to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            socket
                .peer_addr()
                .map(|addr| addr.to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
        );

        socket.set_nodelay(true)?;

        process_socket(socket, server_context.clone()).await;
    }
}

async fn process_socket(socket: TcpStream, server_context: ServerContext) {
    let connection_context = ConnectionContext::new(socket.local_addr().ok(), socket.peer_addr().ok());
    let (readhalf, writehalf) = socket.into_split();
    // channel for result
    let (result_sender, result_receiver) = mpsc::unbounded_channel::<CommandResult>();
    // channel for request
    let (command_sender, command_receiver) = mpsc::unbounded_channel::<RpcCommand>();

    ReadTask::new(
        readhalf,
        command_sender,
        result_sender.clone(),
        server_context.clone(),
        connection_context,
    )
    .spawn();

    VfsTask::new(command_receiver, result_sender, server_context).spawn();

    WriteTask::new(writehalf, result_receiver).spawn();
}
