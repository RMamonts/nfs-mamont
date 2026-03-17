//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

pub mod allocator;
pub mod client;
pub mod consts;
mod context;
pub mod mount;
pub mod parser;
pub mod rpc;
pub mod serializer;
pub mod service;
pub mod task;
pub mod vfs;

use tokio::net::TcpListener;

use crate::task::connection;
use crate::task::global::mount::MountTask;

/// Starts the NFS server and processes client connections.
pub async fn handle_forever(listener: TcpListener) -> std::io::Result<()> {
    // TODO: pass exports from config file
    let (mount_task, mount_sender) = MountTask::new(Vec::new());
    mount_task.spawn();

    loop {
        let (socket, _) = listener.accept().await?;

        socket.set_nodelay(true)?;

        connection::new(socket, mount_sender.clone()).await;
    }
}
