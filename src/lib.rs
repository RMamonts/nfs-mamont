//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

pub mod allocator;
pub mod consts;
mod context;
pub mod mount;
pub mod parser;
pub mod rpc;
pub mod serializer;
pub mod service;
pub mod task;
pub mod vfs;
pub use context::ServerContext;

use tokio::net::TcpListener;

use crate::service::mount::ExportEntryWrapper;
use crate::task::connection;
use crate::task::global::mount::MountTask;

/// Starts the NFS server and processes client connections.
pub async fn handle_forever(listener: TcpListener, context: ServerContext) -> std::io::Result<()> {
    handle_forever_with_exports(listener, context, Vec::new()).await
}

/// Starts the NFS server and processes client connections with explicit MOUNT exports.
pub async fn handle_forever_with_exports(
    listener: TcpListener,
    context: ServerContext,
    exports: Vec<ExportEntryWrapper>,
) -> std::io::Result<()> {
    let export_paths = exports
        .iter()
        .map(|entry| entry.export.directory.as_path().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    dbg!(&format!("server start: configured MOUNT exports={export_paths:?}"));

    let (mount_task, mount_sender) = MountTask::new(exports);
    mount_task.spawn();

    loop {
        let (socket, _) = listener.accept().await?;

        socket.set_nodelay(true)?;

        connection::new(socket, mount_sender.clone(), &context).await;
    }
}
