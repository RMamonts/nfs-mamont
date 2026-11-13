use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use crate::allocator::{Allocator, Buffer};
use crate::backend::BackendRegistry;
use crate::limits::{self, Limits};
use crate::vfs;
use crate::vfs::file::BackendId;

/// Shared server resources: buffer allocator, backends and limits.
///
/// Construct once at startup and share across connection handlers. The context is
/// created **without** backends: they are attached at runtime with [`Self::add_backend`]
/// and detached with [`Self::remove_backend`]. Procedures whose file handle points at a
/// backend that is not attached fail with [`vfs::Error::StaleFile`].
///
/// The `with_*` methods set the [`Limits`] that keep clients from taking more of the
/// server than their share.
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
    /// Allocator backing all user-data buffers: READ output buffers and WRITE
    /// payload buffers are served from this single pool.
    allocator: Arc<A>,
    /// Filesystem implementations backing NFS operations, keyed by backend id.
    backends: BackendRegistry<V>,
    /// Limits of the server, see [`crate::limits`].
    limits: Limits,
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
    pub fn new(allocator: Arc<A>, vfs_concurrency: NonZeroUsize) -> Self {
        Self {
            allocator,
            backends: BackendRegistry::new(),
            limits: limits::defaults(vfs_concurrency),
        }
    }

    /// Returns the limits of the server, set with [`Self::new`] and the `with_*` methods.
    #[inline]
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Limits the number of open connections. At the limit a new connection closes the
    /// oldest one that has not sent a valid call yet, see
    /// [`Self::with_max_unverified_connections`]; without one, it waits until another
    /// connection is closed, and the rest wait in the kernel's accept queue.
    ///
    /// Off by default: an NFS client usually talks to a server over one connection per
    /// machine, or `nconnect` of them, and a limit below their number keeps new clients
    /// from mounting.
    pub fn with_max_connections(mut self, max: NonZeroUsize) -> Self {
        self.limits.max_connections = Some(max);
        self
    }

    /// Limits the connections that have not sent a valid call yet: a new one closes the
    /// oldest of them. Clients that send requests are never affected. The default is
    /// [`crate::consts::connection::DEFAULT_MAX_UNVERIFIED_CONNECTIONS`].
    pub fn with_max_unverified_connections(mut self, max: NonZeroUsize) -> Self {
        self.limits.max_unverified_connections = max;
        self
    }

    /// Closes a connection that has had no request in flight, and no request read or
    /// answered, for `timeout` to twice that, so that the connections of clients that are
    /// gone do not stay open. The default is
    /// [`crate::consts::connection::DEFAULT_IDLE_TIMEOUT`]; a zero `timeout` turns it off.
    /// Clients reconnect when they have a request to send, and with nothing in flight none
    /// has to be sent again.
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.limits.idle_timeout = unless_zero(timeout);
        self
    }

    /// Sets how many requests one connection may have read from the socket but not yet
    /// answered; beyond it the server stops reading and TCP flow control holds the
    /// client back. The default is the larger of
    /// [`crate::consts::connection::DEFAULT_MAX_IN_FLIGHT`] and
    /// [`crate::consts::connection::IN_FLIGHT_PER_VFS_WORKER`] × `vfs_concurrency`.
    ///
    /// The limit caps the throughput of one connection at `max_in_flight / latency`. It
    /// counts requests, not bytes: a client that stops reading its READ replies can still
    /// hold the whole allocator pool, see [`Self::with_max_data_per_connection`].
    pub fn with_max_in_flight(mut self, max_in_flight: NonZeroUsize) -> Self {
        self.limits.max_in_flight = max_in_flight;
        self
    }

    /// Limits the allocator memory the READ and WRITE requests of one connection may
    /// hold, counted by [`Buffer::footprint`], so that a client that stops reading its
    /// READ replies cannot take the whole pool. Off by default.
    ///
    /// The server stops reading the connection while `bytes` are in flight; the request
    /// that crosses the limit is still served. A client needs about `bandwidth × round
    /// trip` bytes in flight to run at line rate, a few MiB on 10 GbE.
    pub fn with_max_data_per_connection(mut self, bytes: NonZeroUsize) -> Self {
        self.limits.max_data_per_connection = Some(bytes);
        self
    }

    /// Closes a connection whose client does not take a reply within `timeout`. Off by
    /// default; a zero `timeout` turns it off.
    ///
    /// Without it, a client that stops reading keeps its connection, its in-flight
    /// slots and their buffers for as long as it stays connected.
    pub fn with_write_timeout(mut self, timeout: Duration) -> Self {
        self.limits.write_timeout = unless_zero(timeout);
        self
    }

    /// Limits how many NFS procedures may wait for or run in the VFS pool at once, over
    /// all connections, so that the queue no longer grows with their number. Off by
    /// default; MOUNT and NLM procedures are not counted. A few slots per VFS worker
    /// keep the pool busy, say `4 × vfs_concurrency`; more only add queueing delay.
    pub fn with_max_pending(mut self, max_pending: NonZeroUsize) -> Self {
        self.limits.max_pending = Some(max_pending);
        self
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

/// Returns `timeout`, or `None` for a zero one, which turns a timeout off.
fn unless_zero(timeout: Duration) -> Option<Duration> {
    (!timeout.is_zero()).then_some(timeout)
}
