//! NFS Mamont - A Network File System (NFS) server implementation in Rust.
#![allow(dead_code)]
mod message_types;
mod read_task;
mod stream_writer;
mod vfs_task;

use tokio::net::TcpListener;
use tokio::net::TcpStream;

use crate::message_types::{create_early_reply_channel, create_proc_channel, create_reply_channel};
use crate::read_task::ReadTask;
use crate::stream_writer::StreamWriter;
use crate::vfs_task::VfsTask;

const DEFAULT_MPSC_SIZE: usize = 65536;

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

    let (proc_sender, proc_recv) = create_proc_channel(DEFAULT_MPSC_SIZE);
    let (reply_sender, reply_recv) = create_reply_channel(DEFAULT_MPSC_SIZE);
    let (early_send, early_recv) = create_early_reply_channel(DEFAULT_MPSC_SIZE);

    ReadTask::spawn(readhalf, proc_sender, early_send);
    VfsTask::spawn(proc_recv, reply_sender);
    StreamWriter::spawn(writehalf, reply_recv, early_recv);
}
