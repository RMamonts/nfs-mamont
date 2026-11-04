//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod allocator;
mod backend;
pub mod consts;
mod context;
mod limits;
pub mod mount;
pub mod nlm;
mod parser;
mod rpc;
mod rpcbind;
mod serializer;
pub mod stats;
mod task;
pub mod vfs;

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::signal;
use tracing::{info, warn};

use crate::consts::socket::{
    ACCEPT_ERROR_BACKOFF, ACCEPT_ERROR_EBADF, ACCEPT_ERROR_EINVAL, ACCEPT_ERROR_ENOTSOCK,
};
use crate::mount::Mount;
use crate::nlm::Nlm;
use crate::task::connection;
use crate::task::global::mount::MountTask;
use crate::task::global::nlm::nlm_task::NlmTask;
use crate::vfs::Vfs;

pub use allocator::{Allocator, Buffer};
pub use backend::BackendRegistry;
pub use context::{default_drr_quantum, default_max_pending, ServerContext};
pub use limits::Scheduling;
pub use rpc::auth;
pub use rpc::AuthFlavor;
pub use vfs::file::BackendId;

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
    let limits = context.limits();

    let accept_result = loop {
        // A slot of the connection budget is taken before `accept`, so that beyond the
        // limit the kernel backlog holds new connections until one is closed.
        let permit = match &limits.connections {
            Some(connections) => {
                let acquire = async {
                    match Arc::clone(connections).try_acquire_owned() {
                        Ok(permit) => Ok(permit),
                        Err(_) => {
                            stats::CONNECTION_WAITS.fetch_add(1, Ordering::Relaxed);
                            Arc::clone(connections).acquire_owned().await
                        }
                    }
                };
                tokio::select! {
                    permit = acquire => match permit {
                        Ok(permit) => Some(permit),
                        Err(_) => break Ok(()),
                    },
                    result = &mut shutdown => {
                        if matches!(result, Ok(())) {
                            break Ok(());
                        }
                        continue;
                    }
                }
            }
            None => None,
        };
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((socket, _)) => {
                        connection::new(
                            socket,
                            mount_sender.clone(),
                            nlm_sender.clone(),
                            &context,
                            permit,
                        );
                    }
                    Err(err) if is_fatal_accept_error(&err) => break Err(err),
                    Err(err) => {
                        warn!(error = %err, "accept failed, retrying");
                        tokio::time::sleep(ACCEPT_ERROR_BACKOFF).await;
                    }
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

/// Returns `true` for `accept` errors after which the listener is unusable.
fn is_fatal_accept_error(err: &std::io::Error) -> bool {
    match err.raw_os_error() {
        Some(errno) => {
            errno == ACCEPT_ERROR_EBADF
                || errno == ACCEPT_ERROR_EINVAL
                || errno == ACCEPT_ERROR_ENOTSOCK
        }
        None => false,
    }
}
