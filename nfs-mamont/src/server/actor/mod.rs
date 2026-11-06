//! Server lifecycle, configuration and accept-loop actor.
//!
//! Bind the TCP listener and construct the shared ServerContext before creating
//! a Server with Server::spawn. Keep its ServerHandle to observe lifecycle changes or request
//! shutdown. Shutdown stops accepting new connections; already accepted connection
//! tasks continue independently. The spawned task reports accept or signal errors.

#[cfg(test)]
mod tests;

use std::{io, net::SocketAddr, sync::Arc, time::Duration};

use async_channel::{Receiver, Sender};
use tokio::{
    net::{TcpListener, TcpStream},
    signal,
    sync::watch,
    task::JoinHandle,
};
use tracing::{info, warn};

use crate::{
    consts::socket::{
        ACCEPT_ERROR_BACKOFF, ACCEPT_ERROR_EBADF, ACCEPT_ERROR_EINVAL, ACCEPT_ERROR_ENOTSOCK,
    },
    mount::Mount,
    nlm::Nlm,
    rpcbind,
    task::{
        connection,
        global::{mount::MountTask, nlm::nlm_task::NlmTask},
    },
    vfs::Vfs,
    Allocator, Buffer,
};

use super::ServerContext;

/// Runtime settings for the server actor. Listener binding, filesystem backends
/// and buffer allocation remain explicit construction dependencies.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Publish NFS/MOUNT/NLM mappings in local rpcbind (best effort).
    pub register_rpcbind: bool,
    /// Pause before retrying a transient listener accept failure.
    pub accept_error_backoff: Duration,
    /// Stop on Ctrl-C. Disable when the embedding application manages signals.
    pub handle_ctrl_c: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            register_rpcbind: true,
            accept_error_backoff: ACCEPT_ERROR_BACKOFF,
            handle_ctrl_c: true,
        }
    }
}

/// Lifecycle of the listening server, independent of accepted connection tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    /// Constructed and bound, but not yet running.
    Ready,
    /// Initializing service publication.
    Starting,
    /// Accepting connections.
    Running,
    /// Accept loop ended; withdrawing service publication.
    Stopping,
    /// Finished successfully.
    Stopped,
    /// Finished with an I/O or signal error, returned by run/spawn.
    Failed,
}

/// Control messages accepted by the server actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerCommand {
    Shutdown,
}

/// Cloneable control and observation endpoints for one server actor.
/// Dropping every handle/sender also requests shutdown. Watch receivers expose
/// the latest state; rapid intermediate transitions may be coalesced.
#[derive(Clone)]
pub struct ServerHandle {
    /// Actual bound listener address, including an OS-assigned port when using 0.
    pub address: SocketAddr,
    pub state: watch::Receiver<ServerState>,
    pub commands: Sender<ServerCommand>,
}

/// Server actor owning its listener, configuration and protocol services.
///
/// ```no_run
/// use std::sync::Arc;
/// use nfs_mamont::{Allocator, Buffer};
/// use nfs_mamont::{Server, ServerCommand, ServerConfig, ServerContext};
/// use nfs_mamont::{mount::Mount, nlm::Nlm, vfs::Vfs};
/// use tokio::net::TcpListener;
///
/// async fn start<A, B, V, M, N>(
///     listener: TcpListener, context: ServerContext<A, V, B>, mount: Arc<M>, nlm: Arc<N>,
/// ) -> Result<(), Box<dyn std::error::Error>>
/// where
///     A: Allocator<Buffer = B> + Send + Sync + 'static,
///     B: Buffer + 'static, V: Vfs<B> + Send + Sync + 'static,
///     M: Mount + Send + Sync + 'static, N: Nlm + Send + Sync + 'static,
/// {
///     let config = ServerConfig { handle_ctrl_c: false, ..ServerConfig::default() };
///     let (handle, running) = Server::spawn(listener, context, mount, nlm, config)?;
///     // Keep handle.state to observe the actor while the application runs.
///     handle.commands.send(ServerCommand::Shutdown).await?;
///     running.await??;
///     Ok(())
/// }
/// ```
pub struct Server<A, B, M, N, V>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: Vfs<B> + Send + Sync + 'static,
{
    listener: TcpListener,
    context: ServerContext<A, V, B>,
    mount_service: Arc<M>,
    nlm_service: Arc<N>,
    config: ServerConfig,
    commands: Receiver<ServerCommand>,
    state: watch::Sender<ServerState>,
}

impl<A, B, M, N, V> Server<A, B, M, N, V>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    M: Mount + Send + Sync + 'static,
    N: Nlm + Send + Sync + 'static,
    V: Vfs<B> + Send + Sync + 'static,
{
    fn new(
        listener: TcpListener,
        context: ServerContext<A, V, B>,
        mount_service: Arc<M>,
        nlm_service: Arc<N>,
        config: ServerConfig,
    ) -> io::Result<(Self, ServerHandle)> {
        let address = listener.local_addr()?;
        let (sender, commands) = async_channel::bounded(1);
        let (state, receiver) = watch::channel(ServerState::Ready);
        let server =
            Self { listener, context, mount_service, nlm_service, config, commands, state };
        let handle = ServerHandle { address, state: receiver, commands: sender };
        Ok((server, handle))
    }

    /// Runs the server on the current task until shutdown or a fatal error.
    async fn run(self) -> io::Result<()> {
        let (mount_task, mount_sender) = MountTask::new(self.mount_service);
        mount_task.spawn();
        let (nlm_task, nlm_sender) = NlmTask::new(self.nlm_service);
        nlm_task.spawn();
        run_listener(self.listener, self.config, self.commands, self.state, move |socket| {
            connection::new(socket, mount_sender.clone(), nlm_sender.clone(), &self.context);
        })
        .await
    }

    /// Creates and spawns the server, returning its control/state handle and the
    /// task carrying its final result. Fails if the listener address is unavailable.
    ///
    /// # Panics
    ///
    /// Panics if called outside a Tokio runtime.
    pub fn spawn(
        listener: TcpListener,
        context: ServerContext<A, V, B>,
        mount_service: Arc<M>,
        nlm_service: Arc<N>,
        config: ServerConfig,
    ) -> io::Result<(ServerHandle, JoinHandle<io::Result<()>>)> {
        let (server, handle) = Self::new(listener, context, mount_service, nlm_service, config)?;
        let running = tokio::spawn(server.run());
        Ok((handle, running))
    }
}

async fn run_listener(
    listener: TcpListener,
    config: ServerConfig,
    commands: Receiver<ServerCommand>,
    state: watch::Sender<ServerState>,
    mut on_connection: impl FnMut(TcpStream),
) -> io::Result<()> {
    state.send_replace(ServerState::Starting);
    let port = match listener.local_addr() {
        Ok(address) => address.port(),
        Err(error) => {
            state.send_replace(ServerState::Failed);
            return Err(error);
        }
    };
    // Registering may partially publish services before cancellation. Always
    // withdraw mappings on exit when publication was enabled.
    let startup_stopped = if config.register_rpcbind {
        tokio::select! {
            result = rpcbind::register(port) => {
                match result {
                    Ok(()) => info!(port, "registered NFS/MOUNT/NLM services with rpcbind"),
                    Err(error) => warn!(port, %error, "rpcbind registration failed; use explicit mount ports"),
                }
                false
            }
            _ = commands.recv() => true,
        }
    } else {
        false
    };
    let result = if startup_stopped {
        Ok(())
    } else {
        state.send_replace(ServerState::Running);
        let shutdown = async {
            if config.handle_ctrl_c {
                signal::ctrl_c().await
            } else {
                std::future::pending::<io::Result<()>>().await
            }
        };
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                accepted = accept_retry(&listener, config.accept_error_backoff) => {
                    match accepted {
                        Ok(socket) => on_connection(socket),
                        Err(error) => break Err(error),
                    }
                }
                _ = commands.recv() => break Ok(()),
                result = &mut shutdown => break result,
            }
        }
    };
    state.send_replace(ServerState::Stopping);
    if config.register_rpcbind {
        let _ = rpcbind::unregister(port).await;
    }
    // Close the listener before publishing the terminal state.
    drop(listener);
    state.send_replace(if result.is_ok() { ServerState::Stopped } else { ServerState::Failed });
    result
}

async fn accept_retry(listener: &TcpListener, backoff: Duration) -> io::Result<TcpStream> {
    loop {
        match listener.accept().await {
            Ok((socket, _)) => return Ok(socket),
            Err(error) if is_fatal_accept_error(&error) => return Err(error),
            Err(error) => {
                warn!(%error, "accept failed, retrying");
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

fn is_fatal_accept_error(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(ACCEPT_ERROR_EBADF | ACCEPT_ERROR_EINVAL | ACCEPT_ERROR_ENOTSOCK)
    )
}
