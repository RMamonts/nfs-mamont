use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use crate::allocator::{Allocator, Buffer};
use crate::backend::BackendRegistry;
use crate::consts::connection::{DEFAULT_MAX_IN_FLIGHT, IN_FLIGHT_PER_VFS_WORKER};
use crate::task::global::vfs::VfsManager;
use crate::vfs;
use crate::vfs::file::BackendId;

/// Shared server resources: vfs manager, buffer allocator, and backends.
///
/// Construct once at startup and share across connection handlers. The context is
/// created **without** backends: they are attached at runtime with [`Self::add_backend`]
/// and detached with [`Self::remove_backend`]. Procedures whose file handle points at a
/// backend that is not attached fail with [`vfs::Error::StaleFile`].
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
    /// Requests one connection may have read but not yet answered.
    max_in_flight: NonZeroUsize,
    /// How long a reply may wait for the socket before the connection is closed.
    write_timeout: Option<Duration>,
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
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn new(allocator: Arc<A>, vfs_concurrency: NonZeroUsize) -> Self {
        let backends = BackendRegistry::new();
        let vfs_manager = VfsManager::new(backends.clone(), vfs_concurrency);
        let max_in_flight = default_max_in_flight(vfs_concurrency);

        Self { vfs_manager, allocator, backends, max_in_flight, write_timeout: None }
    }

    /// Sets how many requests one connection may have read from the socket but not yet
    /// answered; beyond it the server stops reading and TCP flow control holds the
    /// client back. The default is the larger of [`DEFAULT_MAX_IN_FLIGHT`] and
    /// [`IN_FLIGHT_PER_VFS_WORKER`] × `vfs_concurrency`.
    ///
    /// The limit caps the throughput of one connection at `max_in_flight / latency`. It
    /// counts requests, not bytes: a client that stops reading its READ replies can still
    /// hold the whole allocator pool.
    pub fn with_max_in_flight(mut self, max_in_flight: NonZeroUsize) -> Self {
        self.max_in_flight = max_in_flight;
        self
    }

    /// Returns how many requests one connection may have read but not yet answered.
    #[inline]
    pub fn max_in_flight(&self) -> NonZeroUsize {
        self.max_in_flight
    }

    /// Closes a connection whose client does not take a reply within `timeout`. Off by
    /// default; a zero `timeout` turns it off.
    ///
    /// Without it, a client that stops reading keeps its connection, its in-flight
    /// slots and their buffers for as long as it stays connected.
    pub fn with_write_timeout(mut self, timeout: Duration) -> Self {
        self.write_timeout = (!timeout.is_zero()).then_some(timeout);
        self
    }

    /// Returns the write timeout, see [`Self::with_write_timeout`].
    #[inline]
    pub fn write_timeout(&self) -> Option<Duration> {
        self.write_timeout
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

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::default_max_in_flight;
    use crate::consts::connection::DEFAULT_MAX_IN_FLIGHT;

    #[test]
    fn the_default_limit_grows_with_the_pool() {
        let limit = |workers| default_max_in_flight(NonZeroUsize::new(workers).unwrap()).get();

        assert_eq!(limit(1), DEFAULT_MAX_IN_FLIGHT.get());
        assert_eq!(limit(128), DEFAULT_MAX_IN_FLIGHT.get());
        assert_eq!(limit(129), 258);
        assert_eq!(limit(usize::MAX), usize::MAX);
    }
}
