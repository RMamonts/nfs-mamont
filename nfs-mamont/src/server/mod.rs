//! NFS server: [`Server`] and its [`ServerBuilder`].
//!
//! Every run of a server is supervised by [`serve`], which owns the listener and
//! every task the run spawns: the global VFS, MOUNT and NLM tasks and the read/write
//! tasks of each connection. All of them live in a single [`JoinSet`], so none of
//! them outlives the run.

mod builder;
#[cfg(test)]
mod tests;

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::task::{JoinError, JoinHandle, JoinSet};
use tokio::time;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::allocator::{Allocator, Buffer};
use crate::backend::BackendRegistry;
use crate::mount::Mount;
use crate::nlm::Nlm;
use crate::rpcbind;
use crate::service::mount::MountService;
use crate::service::nlm::NlmService;
use crate::task::connection::{self, ConnectionContext};
use crate::task::global::mount::MountTask;
use crate::task::global::nlm::NlmTask;
use crate::task::global::vfs::VfsTask;
use crate::vfs::Vfs;

pub use builder::ServerBuilder;

/// Time a stopping server waits for in-flight requests before aborting its tasks.
pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// NFS server serving NFSv3, MOUNT and NLM over a single TCP port.
///
/// A server is created stopped by [`Server::builder`]. [`Server::start`] binds the
/// listener and spawns the server tasks, [`Server::stop`] shuts them down gracefully
/// and releases the port; after that the server can be started again. Backends and
/// the state of the MOUNT and NLM services persist across restarts.
///
/// Dropping a running server aborts its tasks and closes its connections without
/// waiting for in-flight requests and without withdrawing the rpcbind mappings, so
/// prefer [`Server::stop`].
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
///
/// use nfs_mamont::vfs::Vfs;
/// use nfs_mamont::{Allocator, BackendRegistry, Buffer, Server};
///
/// async fn run<A, B, V>(allocator: Arc<A>, fs: Arc<V>) -> std::io::Result<()>
/// where
///     A: Allocator<Buffer = B> + Send + Sync + 'static,
///     B: Buffer + 'static,
///     V: Vfs<B> + Send + Sync + 'static,
/// {
///     let backends = BackendRegistry::new();
///     backends.add(fs).expect("no free backend slot");
///
///     let mut server = Server::builder(allocator)
///         .bind("0.0.0.0:2049".parse().unwrap())
///         .backends(backends)
///         .build();
///
///     server.start().await?;
///     tokio::signal::ctrl_c().await?;
///     server.stop().await
/// }
/// ```
pub struct Server<A, V, M = MountService, N = NlmService> {
    /// Address the listener is bound to on every start.
    addr: SocketAddr,
    /// Whether the services are published with the local rpcbind.
    rpcbind: bool,
    /// Time [`Server::stop`] waits for in-flight requests.
    shutdown_timeout: Duration,
    /// Allocator backing all user-data buffers: READ output buffers and WRITE
    /// payload buffers are served from this single pool.
    allocator: Arc<A>,
    /// Filesystem implementations backing NFS operations, keyed by backend index.
    backends: BackendRegistry<V>,
    /// Service answering MOUNT requests.
    mount_service: Arc<M>,
    /// Service answering NLM requests.
    nlm_service: Arc<N>,
    /// Current run, [`None`] while the server is stopped.
    running: Option<Running>,
}

/// One run of a [`Server`], from [`Server::start`] to [`Server::stop`].
struct Running {
    /// Address the listener is actually bound to.
    local_addr: SocketAddr,
    /// Whether this run registered its mappings with rpcbind.
    rpcbind_registered: bool,
    /// Cancelled to stop the run.
    shutdown: CancellationToken,
    /// Task running [`serve`].
    supervisor: JoinHandle<io::Result<()>>,
}

impl<A, V> Server<A, V> {
    /// Returns a builder of a server using `allocator` for user-data buffers.
    pub fn builder(allocator: Arc<A>) -> ServerBuilder<A, V> {
        ServerBuilder::new(allocator)
    }
}

impl<A, V, M, N> Server<A, V, M, N> {
    /// Returns `true` while the server is started and accepts connections.
    ///
    /// Turns `false` after [`Server::stop`], and also when accepting connections
    /// failed; [`Server::stop`] then returns that error.
    pub fn is_running(&self) -> bool {
        self.running.as_ref().is_some_and(|running| !running.supervisor.is_finished())
    }

    /// Returns the address the server listens on, or [`None`] if it is not started.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.running.as_ref().map(|running| running.local_addr)
    }

    /// Returns a handle of the backend registry.
    ///
    /// The handle shares the registry with the server, so backends can be attached
    /// and detached at any time, including while the server is running.
    pub fn backends(&self) -> BackendRegistry<V> {
        self.backends.clone()
    }

    /// Returns the service answering MOUNT requests, e.g. to manage exports.
    pub fn mount_service(&self) -> &Arc<M> {
        &self.mount_service
    }

    /// Returns the service answering NLM requests.
    pub fn nlm_service(&self) -> &Arc<N> {
        &self.nlm_service
    }
}

impl<A, B, V, M, N> Server<A, V, M, N>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: Vfs<B> + Send + Sync + 'static,
    M: Mount + Send + Sync + 'static,
    N: Nlm + Send + Sync + 'static,
{
    /// Binds the listener, registers with rpcbind if enabled and spawns the server
    /// tasks.
    ///
    /// # Returns
    ///
    /// The address the server listens on.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::AlreadyExists`] if the server is already started,
    /// otherwise the error of binding the listener.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub async fn start(&mut self) -> io::Result<SocketAddr> {
        if self.running.is_some() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "server is already started"));
        }

        let listener = TcpListener::bind(self.addr).await?;
        let local_addr = listener.local_addr()?;
        let rpcbind_registered = self.rpcbind && register(local_addr.port()).await;

        let shutdown = CancellationToken::new();
        let supervisor = tokio::spawn(serve(
            listener,
            Arc::clone(&self.allocator),
            self.backends.clone(),
            Arc::clone(&self.mount_service),
            Arc::clone(&self.nlm_service),
            shutdown.clone(),
            self.shutdown_timeout,
        ));

        info!(addr = %local_addr, "server started");
        self.running = Some(Running { local_addr, rpcbind_registered, shutdown, supervisor });

        Ok(local_addr)
    }

    /// Stops the server gracefully and releases the port.
    ///
    /// The listener is closed and connections stop taking new requests right away.
    /// Requests already taken run to completion and their replies are sent, then the
    /// connections are closed; tasks still running after the shutdown timeout are
    /// aborted. Stopping a stopped server does nothing.
    ///
    /// # Errors
    ///
    /// Returns the error that made the server stop accepting connections on its own,
    /// if any.
    pub async fn stop(&mut self) -> io::Result<()> {
        let Some(running) = self.running.take() else {
            return Ok(());
        };

        running.shutdown.cancel();
        // The listener is closed right away, so withdraw the mappings while the
        // remaining requests drain.
        if running.rpcbind_registered {
            let _ = rpcbind::unregister(running.local_addr.port()).await;
        }

        let result = running.supervisor.await.unwrap_or_else(|err| Err(io::Error::other(err)));
        info!(addr = %running.local_addr, "server stopped");

        result
    }
}

impl<A, V, M, N> Drop for Server<A, V, M, N> {
    /// Aborts the tasks of a server that was not stopped.
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            // Dropping the supervisor drops its task set, which aborts every task.
            running.supervisor.abort();
        }
    }
}

/// Publishes the services listening on `port` with the local rpcbind.
///
/// # Returns
///
/// Whether the mappings were registered: only then are they withdrawn on stop, so
/// mappings of another server rejecting the registration are left intact.
async fn register(port: u16) -> bool {
    match rpcbind::register(port).await {
        Ok(()) => {
            info!(port, "registered NFS/MOUNT/NLM services with rpcbind");
            true
        }
        Err(err) => {
            warn!(
                port,
                error = %err,
                "failed to register with rpcbind; clients must pass port=/mountport= options"
            );
            false
        }
    }
}

/// Accepts connections on `listener` until `shutdown` is cancelled, then shuts down
/// gracefully.
///
/// Shutdown proceeds as follows:
///
/// 1. The listener is dropped, releasing the port.
/// 2. Read tasks stop taking new requests; a partially read request is discarded.
/// 3. Requests already dispatched run to completion and their replies are written,
///    after which every connection is closed and the global tasks exit.
/// 4. Tasks still running after `shutdown_timeout` are aborted.
///
/// # Returns
///
/// `Ok(())` once every task has finished.
///
/// # Errors
///
/// Returns the error of [`TcpListener::accept`] that stopped the server. The shutdown
/// is still graceful in that case.
pub async fn serve<A, B, V, M, N>(
    listener: TcpListener,
    allocator: Arc<A>,
    backends: BackendRegistry<V>,
    mount_service: Arc<M>,
    nlm_service: Arc<N>,
    shutdown: CancellationToken,
    shutdown_timeout: Duration,
) -> io::Result<()>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: Vfs<B> + Send + Sync + 'static,
    M: Mount + Send + Sync + 'static,
    N: Nlm + Send + Sync + 'static,
{
    let mut tasks = JoinSet::new();

    let (vfs_task, vfs_sender) = VfsTask::new(backends);
    vfs_task.spawn(&mut tasks);

    let (mount_task, mount_sender) = MountTask::new(mount_service);
    mount_task.spawn(&mut tasks);

    let (nlm_task, nlm_sender) = NlmTask::new(nlm_service);
    nlm_task.spawn(&mut tasks);

    let context = ConnectionContext {
        allocator,
        vfs_sender,
        mount_sender,
        nlm_sender,
        shutdown: shutdown.clone(),
    };

    let accept_result = loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => break Ok(()),
            // Reap finished tasks, so the set only holds the live ones.
            Some(joined) = tasks.join_next() => log_task_exit(joined),
            accepted = listener.accept() => match accepted {
                Ok((socket, client_addr)) => {
                    connection::spawn(socket, client_addr, &context, &mut tasks);
                }
                Err(err) => break Err(err),
            },
        }
    };

    drop(listener);
    // Also reached on an accept error: read tasks have to stop either way.
    shutdown.cancel();
    // Global tasks exit once read tasks are gone and these last senders are dropped.
    drop(context);

    if time::timeout(shutdown_timeout, drain(&mut tasks)).await.is_err() {
        warn!(remaining = tasks.len(), "shutdown timeout elapsed, aborting remaining tasks");
        tasks.shutdown().await;
    }

    accept_result
}

/// Waits until every task in `tasks` finishes.
async fn drain(tasks: &mut JoinSet<()>) {
    while let Some(joined) = tasks.join_next().await {
        log_task_exit(joined);
    }
}

/// Reports a task that panicked instead of returning.
fn log_task_exit(joined: Result<(), JoinError>) {
    if let Err(err) = joined {
        if err.is_panic() {
            error!(error = %err, "server task panicked");
        }
    }
}
