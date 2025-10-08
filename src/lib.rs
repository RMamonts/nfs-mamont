//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod read_task;
pub mod vfs;
mod vfs_task;
mod write_task;

use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::read_task::ReadTask;
use crate::vfs::Vfs;
use crate::vfs_task::VfsTask;
use crate::write_task::WriteTask;

/// Starts the NFS server and processes client connections.
pub async fn handle_forever<V: Vfs + 'static>(
    listener: TcpListener,
    vfs: Arc<V>,
) -> std::io::Result<()> {
    loop {
        let (socket, _) = listener.accept().await?;

        socket.set_nodelay(true)?;

        process_socket(socket, Arc::clone(&vfs)).await;
    }
}

async fn process_socket<V: Vfs + 'static>(socket: TcpStream, vfs: Arc<V>) {
    let (readhalf, writehalf) = socket.into_split();
    // channel for result
    let (result_sender, result_receiver) = mpsc::unbounded_channel::<()>();
    // channel for request
    let (command_sender, command_receiver) = mpsc::unbounded_channel::<()>();

    ReadTask::new(readhalf, command_sender).spawn();

    VfsTask::new(command_receiver, result_sender, vfs).spawn();

    WriteTask::new(writehalf, result_receiver).spawn();
}
