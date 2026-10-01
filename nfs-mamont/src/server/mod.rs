//! Supervision of one server run.
//!
//! [`serve`] owns the listener and every task the run spawns: the global VFS, MOUNT
//! and NLM tasks and the read/write tasks of each connection. All of them live in a
//! single [`JoinSet`], so none of them outlives the run.

#[cfg(test)]
mod tests;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::task::{JoinError, JoinSet};
use tokio::time;
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

use crate::allocator::{Allocator, Buffer};
use crate::backend::BackendRegistry;
use crate::mount::Mount;
use crate::nlm::Nlm;
use crate::task::connection::{self, ConnectionContext};
use crate::task::global::mount::MountTask;
use crate::task::global::nlm::NlmTask;
use crate::task::global::vfs::VfsTask;
use crate::vfs::Vfs;

/// Time a stopping server waits for in-flight requests before aborting its tasks.
pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

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
