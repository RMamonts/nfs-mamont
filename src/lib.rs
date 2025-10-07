//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod message_types;
mod read_task;
mod vfs_task;
mod write_task;

use crate::message_types::{EarlyReply, Request, Response};
use crate::read_task::ReadTask;
use crate::vfs_task::VfsTask;
use crate::write_task::WriteTask;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

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

    let (args_send, args_recv) = mpsc::unbounded_channel::<Request>();
    let (reply_send, reply_recv) = mpsc::unbounded_channel::<Response>();
    let (early_send, early_recv) = mpsc::unbounded_channel::<EarlyReply>();

    ReadTask::spawn(readhalf, args_send, early_send);
    VfsTask::spawn(args_recv, reply_send);
    WriteTask::spawn(writehalf, reply_recv, early_recv);
}
