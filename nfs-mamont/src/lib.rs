//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod allocator;
mod backend;
pub mod connection_id;
pub mod consts;
mod context;
pub mod mount;
pub mod nlm;
mod parser;
mod rpc;
mod rpcbind;
mod serializer;
mod task;
pub mod vfs;

use std::io;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{info, warn};

use crate::consts::socket::{
    ACCEPT_ERROR_BACKOFF, ACCEPT_ERROR_EBADF, ACCEPT_ERROR_EINVAL, ACCEPT_ERROR_ENOTSOCK,
};
use crate::mount::Mount;
use crate::nlm::Nlm;
use crate::task::connection;
use crate::task::connection::probation::Probation;
use crate::task::global::mount::MountTask;
use crate::task::global::nlm::nlm_task::NlmTask;
use crate::vfs::nfsv3::Vfs;

pub use allocator::{Allocator, Buffer};
pub use backend::BackendRegistry;
pub use context::ServerContext;
pub use rpc::auth;
pub use rpc::AuthFlavor;
pub use vfs::nfsv3::file::BackendId;

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

    // `Semaphore::new` panics above `MAX_PERMITS`.
    let connections = context
        .max_connections()
        .map(|max| Arc::new(Semaphore::new(max.get().min(Semaphore::MAX_PERMITS))));
    let probation = Probation::new(context.max_unverified_connections());

    // TODO: will be replaced with other solution in future
    let mut shutdown = Box::pin(signal::ctrl_c());

    let accept_result = loop {
        tokio::select! {
            accepted = accept(&listener, connections.as_ref(), &probation) => {
                match accepted {
                    Ok((socket, permit)) => {
                        connection::new(
                            socket,
                            mount_sender.clone(),
                            nlm_sender.clone(),
                            &context,
                            permit,
                            probation.admit(),
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

/// Accepts the next connection and takes a slot of the connection limit for it, if one
/// is set.
///
/// At the limit the oldest connection that has not sent a valid call yet is closed to
/// make room; without one, the new connection waits until another one is closed, and
/// the connections behind it wait in the kernel's accept queue.
async fn accept(
    listener: &TcpListener,
    connections: Option<&Arc<Semaphore>>,
    probation: &Probation,
) -> io::Result<(TcpStream, Option<OwnedSemaphorePermit>)> {
    let (socket, _) = listener.accept().await?;
    let permit = match connections {
        Some(connections) => {
            if connections.available_permits() == 0 {
                probation.evict_oldest();
            }
            Arc::clone(connections).acquire_owned().await.ok()
        }
        None => None,
    };
    Ok((socket, permit))
}

/// Returns `true` for `accept` errors after which the listener is unusable.
fn is_fatal_accept_error(err: &io::Error) -> bool {
    match err.raw_os_error() {
        Some(errno) => {
            errno == ACCEPT_ERROR_EBADF
                || errno == ACCEPT_ERROR_EINVAL
                || errno == ACCEPT_ERROR_ENOTSOCK
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::Semaphore;
    use tokio::time::timeout;

    use super::accept;
    use crate::task::connection::probation::Probation;

    const WAIT: Duration = Duration::from_secs(5);

    /// A listener with three clients waiting to be accepted, a limit of one connection
    /// and an empty probation.
    async fn listener() -> (TcpListener, Vec<TcpStream>, Arc<Semaphore>, Arc<Probation>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut clients = Vec::new();
        for _ in 0..3 {
            clients.push(TcpStream::connect(addr).await.unwrap());
        }
        let probation = Probation::new(NonZeroUsize::new(64).unwrap());
        (listener, clients, Arc::new(Semaphore::new(1)), probation)
    }

    #[tokio::test]
    async fn accept_waits_for_a_free_connection_slot() {
        let (listener, _clients, slots, probation) = listener().await;

        let (_first, permit) =
            timeout(WAIT, accept(&listener, Some(&slots), &probation)).await.unwrap().unwrap();
        // Cancelling the wait drops the second connection; the third one is accepted below.
        let second = accept(&listener, Some(&slots), &probation);
        assert!(
            timeout(Duration::from_millis(50), second).await.is_err(),
            "accepted past the limit"
        );

        drop(permit);
        timeout(WAIT, accept(&listener, Some(&slots), &probation)).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn at_the_limit_a_connection_that_sent_nothing_makes_room() {
        let (listener, _clients, slots, probation) = listener().await;
        let (_first, permit) =
            timeout(WAIT, accept(&listener, Some(&slots), &probation)).await.unwrap().unwrap();

        // What the first connection does: it closes, giving its slot back, once pushed out.
        let ticket = probation.admit();
        tokio::spawn(async move {
            ticket.evicted().await;
            drop(permit);
        });

        timeout(WAIT, accept(&listener, Some(&slots), &probation)).await.unwrap().unwrap();
    }
}
