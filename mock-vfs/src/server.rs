//! Runs an NFS Mamont server backed by [`MockVfs`].
//!
//! The wiring is the one a real deployment uses: a [`PoolAllocator`], a
//! [`ServerContext`] with the mock attached as a backend, the library
//! [`MountService`] exporting the mock root and the library [`NlmService`].

use std::fmt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use nfs_mamont::vfs::file::{BackendId, Path};
use nfs_mamont::{default_max_pending, handle_forever, BackendRegistry, Scheduling, ServerContext};
use nfs_mamont_util::{MountService, NlmService, PoolAllocator};

use crate::config::MockVfsConfig;
use crate::mock::MockVfs;

/// Default number of VFS workers, i.e. the queue depth of the simulated backend.
pub const DEFAULT_CONCURRENCY: usize = 64;
/// Default size of one allocator buffer: 1 MiB.
pub const DEFAULT_BUF_SIZE: usize = 1 << 20;
/// Default number of allocator buffers.
pub const DEFAULT_BUF_COUNT: usize = 64;
/// Default export path handed to the MOUNT service.
pub const DEFAULT_EXPORT: &str = "/";

/// A server-wide budget: off, derived from the pool and the workers, or a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    Off,
    Auto,
    Value(NonZeroUsize),
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Limit::Off => f.write_str("none"),
            Limit::Auto => f.write_str("auto"),
            Limit::Value(value) => write!(f, "{value}"),
        }
    }
}

/// Budgets and scheduling of the server, see the library `ServerContext` builders.
#[derive(Clone, Debug)]
pub struct Limits {
    /// Requests one connection may have in flight; `None` keeps the library default.
    pub max_in_flight: Option<NonZeroUsize>,
    /// Requests parsed but not yet finished, over all connections.
    pub max_pending: Limit,
    /// Open connections.
    pub max_connections: Option<NonZeroUsize>,
    /// Allocator memory in flight, in bytes; `Auto` is 7/8 of the pool.
    pub data_budget: Limit,
    /// Allocator memory one connection may hold, in bytes.
    pub max_data_per_connection: Option<NonZeroUsize>,
    /// How the VFS pool picks the next procedure.
    pub scheduling: Scheduling,
    /// Closes a connection whose client does not take a reply in time; zero is off.
    pub write_timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_in_flight: None,
            max_pending: Limit::Off,
            max_connections: None,
            data_budget: Limit::Off,
            max_data_per_connection: None,
            scheduling: Scheduling::Fifo,
            write_timeout: Duration::ZERO,
        }
    }
}

/// Everything needed to start a mock server.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Address to listen on. Port `0` picks a free port, see [`Server::addr`].
    pub bind: SocketAddr,
    /// Path clients mount; the MOUNT service answers `MNT` for this path only.
    pub export: String,
    /// Configuration of the mock backend.
    pub vfs: MockVfsConfig,
    /// Number of VFS workers: at most this many procedures run against the backend
    /// at the same time, the rest queue up.
    pub concurrency: NonZeroUsize,
    /// Size of one buffer of the server allocator.
    pub buf_size: NonZeroUsize,
    /// Number of buffers of the server allocator.
    pub buf_count: NonZeroUsize,
    /// Budgets and scheduling.
    pub limits: Limits,
}

impl Default for ServerConfig {
    /// Loopback on a random port, 64 workers, 64 buffers of 1 MiB, export `/`.
    fn default() -> Self {
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            export: DEFAULT_EXPORT.to_string(),
            vfs: MockVfsConfig::default(),
            concurrency: NonZeroUsize::new(DEFAULT_CONCURRENCY).expect("non-zero constant"),
            buf_size: NonZeroUsize::new(DEFAULT_BUF_SIZE).expect("non-zero constant"),
            buf_count: NonZeroUsize::new(DEFAULT_BUF_COUNT).expect("non-zero constant"),
            limits: Limits::default(),
        }
    }
}

impl ServerConfig {
    /// Total capacity of the allocator, the largest transfer the server can serve.
    pub fn allocator_capacity(&self) -> u64 {
        self.buf_size.get() as u64 * self.buf_count.get() as u64
    }

    /// Checks that the configuration is self-consistent.
    pub fn validate(&self) -> io::Result<()> {
        if u64::from(self.vfs.max_io) > self.allocator_capacity() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "max I/O size {} exceeds the allocator capacity {} (buffer size x count)",
                    self.vfs.max_io,
                    self.allocator_capacity()
                ),
            ));
        }
        Path::new(self.export.clone())?;

        Ok(())
    }
}

/// A running mock server.
///
/// Dropping it aborts the accept loop; connections already accepted keep running
/// until their tasks finish.
pub struct Server {
    addr: SocketAddr,
    backend: Arc<MockVfs>,
    backend_id: BackendId,
    backends: BackendRegistry<MockVfs>,
    task: JoinHandle<io::Result<()>>,
}

impl Server {
    /// Binds the listener, builds the server and spawns [`handle_forever`].
    ///
    /// # Panics
    ///
    /// If called outside of a tokio runtime.
    pub async fn start(config: ServerConfig) -> io::Result<Server> {
        config.validate()?;

        let listener = TcpListener::bind(config.bind).await?;
        let addr = listener.local_addr()?;

        let allocator = Arc::new(PoolAllocator::new(config.buf_size, config.buf_count));
        let mut context = ServerContext::new(allocator, config.concurrency);
        let limits = &config.limits;
        if let Some(limit) = limits.max_in_flight {
            context = context.with_max_in_flight(limit);
        }
        let max_in_flight = context.max_in_flight();
        let max_pending = match limits.max_pending {
            Limit::Off => None,
            Limit::Auto => Some(default_max_pending(config.concurrency, max_in_flight)),
            Limit::Value(value) => Some(value),
        };
        if let Some(limit) = max_pending {
            context = context.with_max_pending(limit);
        }
        if let Some(limit) = limits.max_connections {
            context = context.with_max_connections(limit);
        }
        let pool = usize::try_from(config.allocator_capacity()).map_err(io::Error::other)?;
        let data_budget = match limits.data_budget {
            Limit::Off => None,
            Limit::Auto => NonZeroUsize::new(pool / 8 * 7),
            Limit::Value(value) => Some(value),
        };
        if let Some(budget) = data_budget {
            context = context.with_data_budget(budget);
        }
        if let Some(limit) = limits.max_data_per_connection {
            context = context.with_max_data_per_connection(limit);
        }
        context = context.with_scheduling(limits.scheduling);
        if !limits.write_timeout.is_zero() {
            context = context.with_write_timeout(limits.write_timeout);
        }
        eprintln!(
            "limits: max_in_flight={} max_pending={} max_connections={} data_budget={} \
             max_data_per_conn={} scheduler={:?} write_timeout={:?}",
            max_in_flight,
            max_pending.map_or("none".to_string(), |v| v.to_string()),
            limits.max_connections.map_or("none".to_string(), |v| v.to_string()),
            data_budget.map_or("none".to_string(), |v| v.to_string()),
            limits.max_data_per_connection.map_or("none".to_string(), |v| v.to_string()),
            limits.scheduling,
            limits.write_timeout,
        );
        let backend = Arc::new(MockVfs::new(config.vfs));
        let backend_id = context
            .add_backend(Arc::clone(&backend))
            .ok_or_else(|| io::Error::other("no free backend slot"))?;
        let backends = context.backends();

        let mount_service = Arc::new(MountService::new());
        mount_service.add_export(Path::new(config.export)?, MockVfs::root_handle(backend_id)).await;
        let nlm_service = Arc::new(NlmService::new());

        let task = tokio::spawn(handle_forever(listener, context, mount_service, nlm_service));

        Ok(Server { addr, backend, backend_id, backends, task })
    }

    /// Returns the address the server listens on.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Returns the mock backend.
    pub fn backend(&self) -> &Arc<MockVfs> {
        &self.backend
    }

    /// Returns the index the backend is registered under.
    pub fn backend_id(&self) -> BackendId {
        self.backend_id
    }

    /// Returns the registry of backends, to attach or detach backends at runtime.
    pub fn backends(&self) -> &BackendRegistry<MockVfs> {
        &self.backends
    }

    /// Waits until the server stops, which happens on `Ctrl+C` or a fatal accept error.
    pub async fn wait(mut self) -> io::Result<()> {
        let result = (&mut self.task).await;
        result.map_err(io::Error::other)?
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
