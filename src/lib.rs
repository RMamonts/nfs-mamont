//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

pub mod allocator;
pub mod mount;
pub mod nfsv3;
pub mod parser;
mod read_task;
mod rpc;
pub mod vfs;
mod vfs_task;
mod write_task;

use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::read_task::ReadTask;
use crate::vfs_task::VfsTask;
use crate::write_task::WriteTask;

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
