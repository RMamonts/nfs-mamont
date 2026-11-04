use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tracing::warn;

use crate::allocator::{Allocator, Buffer};
use crate::backend::BackendRegistry;
use crate::consts::connection::{DEFAULT_MAX_IN_FLIGHT, IN_FLIGHT_PER_VFS_WORKER};
use crate::limits::{DataBudget, Limits, Scheduling};
use crate::task::global::vfs::VfsManager;
use crate::vfs;
use crate::vfs::file::BackendId;

/// Shared server resources: vfs manager, buffer allocator, backends and the limits.
///
/// Construct once at startup and share across connection handlers. The context is
/// created **without** backends: they are attached at runtime with [`Self::add_backend`]
/// and detached with [`Self::remove_backend`]. Procedures whose file handle points at a
/// backend that is not attached fail with [`vfs::Error::StaleFile`].
///
/// The number of requests one connection may have in flight is set with
/// [`Self::with_max_in_flight`]; the server-wide budgets with
/// [`Self::with_max_connections`], [`Self::with_max_pending`],
/// [`Self::with_data_budget`] and [`Self::with_max_data_per_connection`]; the order
/// the VFS pool serves connections in with [`Self::with_scheduling`].
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
    /// Filesystem implementations backing NFS operations, keyed by backend id.
    backends: BackendRegistry<V>,
    /// Number of VFS workers, i.e. procedures executed against the backends at once.
    vfs_concurrency: NonZeroUsize,
    /// How the VFS pool picks the next procedure.
    scheduling: Scheduling,
    /// Per-connection and server-wide budgets, shared with every connection.
    limits: Arc<Limits>,
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
    /// [`Self::with_max_in_flight`]. No server-wide budget is set and the pool serves
    /// requests in arrival order ([`Scheduling::Fifo`]).
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn new(allocator: Arc<A>, vfs_concurrency: NonZeroUsize) -> Self {
        let backends = BackendRegistry::new();
        let scheduling = Scheduling::Fifo;
        let vfs_manager = VfsManager::new(backends.clone(), vfs_concurrency, scheduling);
        let max_in_flight = default_max_in_flight(vfs_concurrency);
        let limits = Arc::new(Limits::new(max_in_flight, allocator.granularity()));

        Self { vfs_manager, allocator, backends, vfs_concurrency, scheduling, limits }
    }

    fn limits_mut(&mut self) -> &mut Limits {
        Arc::get_mut(&mut self.limits)
            .expect("the limits are shared only with connections, which do not exist yet")
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
    /// `max_in_flight` replies queued; the global command queues hold at most
    /// `max_in_flight × open connections` requests, or `max_pending` when
    /// [`Self::with_max_pending`] is set.
    ///
    /// The limit counts requests, not bytes. It bounds the replies queued by one
    /// connection, about `max_in_flight × reply size` of heap memory, but not the share
    /// of the buffer allocator the connection holds: every non-empty READ and WRITE takes
    /// at least one allocator buffer when it is parsed and keeps it until its reply is
    /// written (READ) or its data is handed to the backend (WRITE). One connection may
    /// thus hold up to `max_in_flight` buffers, so a client that stops reading its READ
    /// replies can take the whole pool and stall READ and WRITE of every other
    /// connection, unless [`Self::with_max_data_per_connection`] bounds it.
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
        self.limits_mut().max_in_flight = max_in_flight;
        self
    }

    /// Limits the number of open connections: beyond it the server stops accepting
    /// until a connection is closed, and the kernel backlog holds the rest.
    pub fn with_max_connections(mut self, max_connections: NonZeroUsize) -> Self {
        self.limits_mut().connections = Some(Arc::new(Semaphore::new(max_connections.get())));
        self
    }

    /// Limits the number of requests parsed by the read tasks but not yet finished by
    /// the VFS, MOUNT or NLM tasks, over all connections.
    ///
    /// A read task that has parsed a request waits for a slot before queueing it, so
    /// the global queues never hold more than `max_pending` requests whatever the
    /// number of connections; slots are handed out in round robin over the waiting
    /// connections. By Little's law a few slots per VFS worker keep the pool busy, say
    /// `4 × vfs_concurrency`; more only adds queueing delay. The limit should not be
    /// below [`Self::max_in_flight`], otherwise a single connection cannot keep the
    /// pool busy.
    pub fn with_max_pending(mut self, max_pending: NonZeroUsize) -> Self {
        if max_pending < self.limits.max_in_flight {
            warn!(
                max_pending,
                max_in_flight = self.limits.max_in_flight,
                "pending limit is below the in-flight limit of one connection"
            );
        }
        self.limits_mut().pending = Some(Arc::new(Semaphore::new(max_pending.get())));
        self
    }

    /// Limits the allocator memory held by READ and WRITE requests in flight, over all
    /// connections, to `bytes` rounded down to the allocator granularity.
    ///
    /// The budget is charged before the allocator is asked, so with a budget below the
    /// pool size the allocator never has to wait: read tasks wait on the budget
    /// instead, in round robin over the connections, and a client that holds buffers
    /// is bounded by [`Self::with_max_data_per_connection`]. A budget above the pool
    /// size is accepted but logged, since the allocator then becomes the limit again.
    pub fn with_data_budget(mut self, bytes: NonZeroUsize) -> Self {
        let granularity = self.limits.granularity.get();
        let units = (bytes.get() / granularity).max(1);
        if bytes > self.allocator.capacity() {
            warn!(
                data_budget = bytes,
                capacity = self.allocator.capacity(),
                "data budget is larger than the allocator capacity: the allocator may \
                 have to wait"
            );
        }
        self.limits_mut().data =
            Some(DataBudget { semaphore: Arc::new(Semaphore::new(units)), units });
        self
    }

    /// Limits the allocator memory held by the READ and WRITE requests of one
    /// connection to `bytes` (rounded up to the allocator granularity per request).
    ///
    /// A client needs about `bandwidth × (round trip + server latency)` bytes in
    /// flight to run at line rate, a few MiB on 10 GbE. A single request larger than
    /// the limit is still served when the connection holds nothing else.
    pub fn with_max_data_per_connection(mut self, bytes: NonZeroUsize) -> Self {
        let bytes = u64::try_from(bytes.get()).expect("usize fits into u64");
        self.limits_mut().max_data_per_connection = Some(bytes);
        self
    }

    /// Sets how the VFS pool picks the next procedure to execute, see [`Scheduling`].
    ///
    /// The workers are restarted with the new queue; call it before the context is
    /// used by [`crate::handle_forever`].
    pub fn with_scheduling(mut self, scheduling: Scheduling) -> Self {
        if scheduling != self.scheduling {
            self.vfs_manager =
                VfsManager::new(self.backends.clone(), self.vfs_concurrency, scheduling);
            self.scheduling = scheduling;
        }
        self
    }

    /// Closes a connection whose client does not take a reply within `timeout`.
    ///
    /// A client that stops reading its replies keeps its in-flight slots, its data
    /// and the allocator buffers of its READ replies for as long as the connection
    /// lives; the timeout bounds that. The Linux kernel NFS server does the same with
    /// a 30 s send timeout. A zero duration disables the timeout.
    pub fn with_write_timeout(mut self, timeout: Duration) -> Self {
        self.limits_mut().write_timeout = (!timeout.is_zero()).then_some(timeout);
        self
    }

    /// Returns how many requests one connection may have read but not yet answered,
    /// see [`Self::with_max_in_flight`].
    #[inline]
    pub fn max_in_flight(&self) -> NonZeroUsize {
        self.limits.max_in_flight
    }

    /// Returns how the VFS pool picks the next procedure, see [`Self::with_scheduling`].
    #[inline]
    pub fn scheduling(&self) -> Scheduling {
        self.scheduling
    }

    /// Returns the budgets shared with the connections.
    #[inline]
    pub(crate) fn limits(&self) -> Arc<Limits> {
        Arc::clone(&self.limits)
    }

    /// Attaches `backend` at runtime, making it serve requests immediately.
    ///
    /// # Returns
    ///
    /// The id (slot and generation) the backend was registered under --- the same id the
    /// backend must encode into the file handles it returns --- or [`None`] if no free
    /// slot is left.
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

/// Returns the default global pending limit for a pool of `vfs_concurrency` workers
/// and a per-connection limit of `max_in_flight`: [`PENDING_PER_VFS_WORKER`] slots per
/// worker, but not fewer than one connection may have in flight.
///
/// [`PENDING_PER_VFS_WORKER`]: crate::consts::connection::PENDING_PER_VFS_WORKER
pub fn default_max_pending(
    vfs_concurrency: NonZeroUsize,
    max_in_flight: NonZeroUsize,
) -> NonZeroUsize {
    max_in_flight
        .max(vfs_concurrency.saturating_mul(crate::consts::connection::PENDING_PER_VFS_WORKER))
}

/// Returns the default quantum of [`Scheduling::Drr`], see
/// [`crate::consts::connection::DEFAULT_DRR_QUANTUM`].
pub fn default_drr_quantum() -> NonZeroU32 {
    crate::consts::connection::DEFAULT_DRR_QUANTUM
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::{default_max_in_flight, default_max_pending};
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

    #[test]
    fn the_pending_default_covers_one_connection() {
        assert_eq!(default_max_pending(nonzero(64), nonzero(256)), nonzero(256));
        assert_eq!(default_max_pending(nonzero(128), nonzero(256)), nonzero(512));
        assert_eq!(default_max_pending(nonzero(16), nonzero(1024)), nonzero(1024));
    }
}
