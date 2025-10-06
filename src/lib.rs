//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod read_task;
mod vfs_task;
mod write_task;

use tokio::net::TcpListener;
use tokio::net::TcpStream;

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

    let (write_half, _join_handle, read_half) = VfsTask::spawn();

    ReadTask::spawn(readhalf, write_half);
    WriteTask::spawn(writehalf, read_half);
}
