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
use tracing::debug;
#[cfg(debug_assertions)]
use tracing_subscriber::EnvFilter;

use crate::service::mount::ExportEntryWrapper;
use crate::task::connection;
use crate::task::global::mount::MountTask;

/// Initializes tracing logs.
///
/// In debug builds logs are enabled by default. In release builds this is a no-op.
#[cfg(debug_assertions)]
pub fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("nfs_mamont=debug"));

    let _ = tracing_subscriber::fmt().with_env_filter(env_filter).try_init();
}

/// Initializes tracing logs.
///
/// In release builds logs are disabled to avoid runtime overhead.
#[cfg(not(debug_assertions))]
pub fn init_tracing() {}

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
    debug!(configured_mount_exports = ?export_paths, "server start: configured MOUNT exports");

    let (mount_task, mount_sender) = MountTask::new(exports);
    mount_task.spawn();

    loop {
        let (socket, _) = listener.accept().await?;

        socket.set_nodelay(true)?;

        connection::new(socket, mount_sender.clone(), &context).await;
    }
}
