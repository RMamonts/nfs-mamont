//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod allocator;
pub mod consts;
mod context;
pub mod mount;
#[allow(dead_code)]
mod nlm;
mod parser;
mod rpc;
mod serializer;
pub mod service;
mod task;
pub mod vfs;

use std::sync::Arc;

use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::task::global::mount::MountTask;
use crate::task::global::nlm::NlmTask;
use crate::vfs::Vfs;
use crate::{mount::Mount, task::connection};

pub use allocator::{Allocator, AllocatorState, Impl, Slice};
pub use context::ServerContext;

/// Initializes tracing logs.
///
/// In debug builds logs are enabled by default. In release builds this is a no-op.
pub fn init_tracing() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("nfs_mamont=debug"));

    let _ = tracing_subscriber::fmt().with_env_filter(env_filter).try_init();
}

/// Starts the NFS server and processes client connections with explicit MOUNT exports.
pub async fn handle_forever<A, M, V>(
    listener: TcpListener,
    context: ServerContext<A, V>,
    mount_service: Arc<M>,
) -> std::io::Result<()>
where
    A: Allocator + Send + Sync + 'static,
    M: Mount + Send + Sync + 'static,
    V: Vfs + Send + Sync + 'static,
{
    let (mount_task, mount_sender) = MountTask::new(mount_service);
    mount_task.spawn();

    let (nlm_task, nlm_sender) = NlmTask::new();
    nlm_task.spawn();

    loop {
        let (socket, _) = listener.accept().await?;

        connection::new(socket, mount_sender.clone(), nlm_sender.clone(), &context).await;
    }
}
