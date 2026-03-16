//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

pub mod allocator;
pub mod client;
mod context;
pub mod mount;
pub mod nfsv3;
pub mod parser;
pub mod rpc;
pub mod serializer;
pub mod service;
pub mod task;
pub mod vfs;

use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::task::connection;
use crate::task::global::mount::{MountCommand, MountTask};

/// Starts the NFS server and processes client connections.
pub async fn handle_forever(listener: TcpListener) -> std::io::Result<()> {
    let (mount_task, mount_sender) = MountTask::new();
    mount_task.spawn();

    loop {
        let (socket, _) = listener.accept().await?;

        socket.set_nodelay(true)?;

        process_socket(socket, mount_sender.clone()).await;
    }
}

async fn process_socket(socket: TcpStream, mount_sender: mpsc::UnboundedSender<MountCommand>) {
    connection::new(socket, mount_sender);
}
