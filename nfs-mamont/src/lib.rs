//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod allocator;
pub mod auth;
pub mod consts;
mod context;
pub mod mount;
#[allow(dead_code)]
mod nlm;
mod parser;
mod rpc;
mod rpcbind;
mod serializer;
pub mod service;
mod task;
pub mod vfs;

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::nlm::Nlm;
use crate::task::global::mount::MountTask;
use crate::task::global::nlm::NlmTask;
use crate::vfs::Vfs;
use crate::{mount::Mount, task::connection};

pub use allocator::{Allocator, Buffer, Impl, Slice, UnownedBuffer};
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
pub async fn handle_forever<A, B, M, N, V>(
    listener: TcpListener,
    context: ServerContext<A, V, B>,
    mount_service: Arc<M>,
    nlm_service: Arc<N>,
) -> std::io::Result<()>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    M: Mount + Send + Sync + 'static,
    N: Nlm + Send + Sync + 'static,
    V: Vfs<B> + Send + Sync + 'static,
{
    let (mount_task, mount_sender) = MountTask::new(mount_service);
    mount_task.spawn();

    let (nlm_task, nlm_sender) = NlmTask::new(nlm_service);
    nlm_task.spawn();

    // Publish the NFS/MOUNT/NLM services to the local rpcbind
    let port = listener.local_addr()?.port();
    match rpcbind::register(port).await {
        Ok(()) => info!(port, "registered NFS/MOUNT/NLM services with rpcbind"),
        Err(err) => warn!(
            port,
            error = %err,
            "failed to register with rpcbind; clients must pass port=/mountport= options"
        ),
    }

    // TODO: will be replaced with other solution in future
    let mut shutdown = Box::pin(signal::ctrl_c());

    let accept_result = loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((socket, _)) => {
                        connection::new(socket, mount_sender.clone(), nlm_sender.clone(), &context).await;
                    }
                    Err(err) => break Err(err),
                }
            }
            result = &mut shutdown => {
                if matches!(result, Ok(())) {
                    break Ok(());
                }
            }
        }
    };

    let _ = rpcbind::unregister(port).await;

    accept_result
}
