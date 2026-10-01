//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod allocator;
mod backend;
pub mod consts;
mod context;
pub mod mount;
#[allow(dead_code)]
mod nlm;
mod parser;
mod rpc;
mod rpcbind;
mod serializer;
mod server;
pub mod service;
mod task;
pub mod vfs;

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::mount::Mount;
use crate::nlm::Nlm;
use crate::vfs::Vfs;

pub use allocator::{Allocator, Buffer, Impl, Slice, UnownedBuffer};
pub use backend::BackendRegistry;
pub use context::ServerContext;
pub use rpc::auth;
pub use vfs::file::BackendId;

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

    let shutdown = CancellationToken::new();
    let serving = server::serve(
        listener,
        context.get_allocator(),
        context.backends(),
        mount_service,
        nlm_service,
        shutdown.clone(),
        server::DEFAULT_SHUTDOWN_TIMEOUT,
    );
    tokio::pin!(serving);

    // TODO: will be replaced with other solution in future
    let result = tokio::select! {
        result = &mut serving => result,
        signal = signal::ctrl_c() => {
            match signal {
                Ok(()) => shutdown.cancel(),
                Err(err) => warn!(error = %err, "cannot listen for Ctrl-C, the server keeps running"),
            }
            serving.await
        }
    };

    let _ = rpcbind::unregister(port).await;

    result
}
