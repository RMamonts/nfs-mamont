use std::num::NonZeroUsize;
use std::sync::Arc;

use tracing::warn;

use crate::allocator::{Allocator, Buffer};
use crate::backend::BackendRegistry;
use crate::consts::connection::{DEFAULT_MAX_IN_FLIGHT, IN_FLIGHT_PER_VFS_WORKER};
use crate::task::global::vfs::VfsManager;
use crate::vfs;
use crate::vfs::file::BackendId;

/// Shared server resources: vfs manager, buffer allocator, backends and the
/// per-connection limits.
///
/// Construct once at startup and share across connection handlers. The context is
/// created **without** backends: they are attached at runtime with [`Self::add_backend`]
/// and detached with [`Self::remove_backend`]. Procedures whose file handle points at a
/// backend that is not attached fail with [`vfs::Error::StaleFile`].
///
/// The number of requests one connection may have in flight is set with
/// [`Self::with_max_in_flight`].
///
/// # Example
///
/// ```no_run
/// use std::num::NonZeroUsize;
/// use std::sync::Arc;
///
/// use nfs_mamont::vfs::Vfs;
/// use nfs_mamont::{Allocator, Buffer, ServerContext};
///
/// fn attach_and_detach<A, B, V>(allocator: Arc<A>, fs: Arc<V>) -> Option<Arc<V>>
/// where
///     A: Allocator<Buffer = B> + Send + Sync + 'static,
///     B: Buffer + 'static,
///     V: Vfs<B> + Send + Sync + 'static,
/// {
///     // Run up to 64 procedures against the backends at the same time.
///     let vfs_concurrency = NonZeroUsize::new(64).unwrap();
///     let context = ServerContext::new(allocator, vfs_concurrency);
///     // Keep a registry handle before the context is moved into the server task.
///     let backends = context.backends();
///
///     let id = backends.add(fs).expect("no free backend slot");
///     backends.remove(id)
/// }
/// ```
pub struct ServerContext<A, V, B>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Pool of workers executing NFS procedures against [`crate::vfs::Vfs`] backends.
    vfs_manager: VfsManager<B>,
    /// Allocator backing all user-data buffers: READ output buffers and WRITE
    /// payload buffers are served from this single pool.
    allocator: Arc<A>,
    /// Filesystem implementations backing NFS operations, keyed by backend index.
    backends: BackendRegistry<V>,
    /// Number of VFS workers, i.e. procedures executed against the backends at once.
    vfs_concurrency: NonZeroUsize,
    /// Requests one connection may have read but not yet answered.
    max_in_flight: NonZeroUsize,
}

impl<A, V, B> ServerContext<A, V, B>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Creates a context with the given allocator and no backends attached.
    ///
    /// `vfs_concurrency` is the maximum number of NFS procedures executed against the
    /// backends at the same time; procedures beyond it wait in a queue. Set it to the
    /// number of operations the backends can run in parallel (their queue depth).
    ///
    /// The per-connection in-flight limit defaults to the larger of
    /// [`DEFAULT_MAX_IN_FLIGHT`] and [`IN_FLIGHT_PER_VFS_WORKER`] × `vfs_concurrency`,
    /// so that a single connection can keep the whole pool busy; see
    /// [`Self::with_max_in_flight`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn new(allocator: Arc<A>, vfs_concurrency: NonZeroUsize) -> Self {
        let backends = BackendRegistry::new();
        let vfs_manager = VfsManager::new(backends.clone(), vfs_concurrency);
        let max_in_flight = default_max_in_flight(vfs_concurrency);

        Self { vfs_manager, allocator, backends, vfs_concurrency, max_in_flight }
    }

    /// Sets how many requests one connection may have read from the socket but not yet
    /// answered. The default is the larger of [`DEFAULT_MAX_IN_FLIGHT`] and
    /// [`IN_FLIGHT_PER_VFS_WORKER`] × `vfs_concurrency`, see [`Self::new`].
    ///
    /// While a connection has `max_in_flight` requests parsed whose replies are not yet
    /// written to the socket, its read task does not read the next request: the server
    /// stops acknowledging the client's data (TCP flow control) and the RPC slot table of
    /// the client waits. Replies are never dropped, and a slow client never blocks the
    /// shared VFS/MOUNT/NLM tasks, because no connection ever has more than
    /// `max_in_flight` replies queued. The memory held by one connection is therefore
    /// bounded by `max_in_flight × reply size`, and the global command queues by
    /// `max_in_flight × open connections`.
    ///
    /// A client needs about `throughput × server latency` requests in flight to keep the
    /// server busy, so a limit below that caps the throughput of one connection at
    /// `max_in_flight / latency`. A limit below `vfs_concurrency` in particular leaves
    /// part of the VFS pool idle for a single client; it is accepted, since it may be
    /// meant to share the pool between many connections, but logged as a warning.
    pub fn with_max_in_flight(mut self, max_in_flight: NonZeroUsize) -> Self {
        if max_in_flight < self.vfs_concurrency {
            warn!(
                max_in_flight,
                vfs_concurrency = self.vfs_concurrency,
                "in-flight limit is below the VFS pool size: one connection cannot keep \
                 all VFS workers busy"
            );
        }
        self.max_in_flight = max_in_flight;
        self
    }

    /// Returns how many requests one connection may have read but not yet answered,
    /// see [`Self::with_max_in_flight`].
    #[inline]
    pub fn max_in_flight(&self) -> NonZeroUsize {
        self.max_in_flight
    }

    /// Attaches `backend` at runtime, making it serve requests immediately.
    ///
    /// # Returns
    ///
    /// The index the backend was registered under --- the same index the backend must
    /// encode into the file handles it returns --- or [`None`] if no free slot is left.
    pub fn add_backend(&self, backend: Arc<V>) -> Option<BackendId> {
        self.backends.add(backend)
    }

    /// Detaches the backend registered under `id`.
    ///
    /// # Returns
    ///
    /// The detached backend, or [`None`] if `id` was not registered.
    pub fn remove_backend(&self, id: BackendId) -> Option<Arc<V>> {
        self.backends.remove(id)
    }

    /// Returns the shared vfs manager used to provide link to NFS procedures processor.
    #[inline]
    pub fn get_vfs_manager(&self) -> &VfsManager<B> {
        &self.vfs_manager
    }

    /// Returns a clone of the [`vfs::Vfs`] backend registered under `id`, if any.
    #[inline]
    pub fn get_backend(&self, id: BackendId) -> Option<Arc<V>> {
        self.backends.get(id)
    }

    /// Returns a clone of the backend registry.
    ///
    /// The clone shares the same map, so it can be used to attach and detach backends
    /// after the context has been moved into [`crate::handle_forever`].
    #[inline]
    pub fn backends(&self) -> BackendRegistry<V> {
        self.backends.clone()
    }

    /// Returns a clone of the shared buffer allocator.
    #[inline]
    pub fn get_allocator(&self) -> Arc<A> {
        Arc::clone(&self.allocator)
    }
}

/// Returns the default in-flight limit for a pool of `vfs_concurrency` workers, see
/// [`ServerContext::new`].
fn default_max_in_flight(vfs_concurrency: NonZeroUsize) -> NonZeroUsize {
    DEFAULT_MAX_IN_FLIGHT.max(vfs_concurrency.saturating_mul(IN_FLIGHT_PER_VFS_WORKER))
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::default_max_in_flight;
    use crate::consts::connection::DEFAULT_MAX_IN_FLIGHT;

    fn nonzero(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    #[test]
    fn small_pools_get_the_default_limit() {
        assert_eq!(default_max_in_flight(nonzero(1)), DEFAULT_MAX_IN_FLIGHT);
        assert_eq!(default_max_in_flight(nonzero(128)), DEFAULT_MAX_IN_FLIGHT);
    }

    #[test]
    fn large_pools_get_two_requests_per_worker() {
        assert_eq!(default_max_in_flight(nonzero(129)), nonzero(258));
        assert_eq!(default_max_in_flight(nonzero(1024)), nonzero(2048));
    }

    #[test]
    fn the_default_limit_saturates() {
        assert_eq!(default_max_in_flight(nonzero(usize::MAX)), nonzero(usize::MAX));
    }
}
