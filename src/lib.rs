//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

pub mod allocator;
pub mod client;
mod context;
pub mod mount;
pub mod nfsv3;
pub mod parser;
pub mod rpc;
pub mod serializer;
pub mod task;
pub mod vfs;

use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::task::connection::read::ReadTask;
use crate::task::connection::vfs::VfsTask;
use crate::task::connection::write::WriteTask;

/// Starts the NFS server and processes client connections.
pub async fn handle_forever(listener: TcpListener) -> std::io::Result<()> {
    loop {
        let (socket, _) = listener.accept().await?;

        socket.set_nodelay(true)?;

        process_socket(socket).await;
    }
}

async fn process_socket(socket: TcpStream) {
    let (readhalf, writehalf) = socket.into_split();
    // channel for result
    let (result_sender, result_receiver) = mpsc::unbounded_channel::<()>();
    // channel for request
    let (command_sender, command_receiver) = mpsc::unbounded_channel::<()>();

    ReadTask::new(readhalf, command_sender).spawn();

    VfsTask::new(command_receiver, result_sender).spawn();

    WriteTask::new(writehalf, result_receiver).spawn();
}
