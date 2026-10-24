//! Runs an NFS Mamont server backed by [`MockVfs`].
//!
//! The wiring is the one a real deployment uses: an [`Impl`] allocator, a
//! [`ServerContext`] with the mock attached as a backend, the library
//! [`MountService`] exporting the mock root and the library [`NlmService`].

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use nfs_mamont::mount::ExportEntry;
use nfs_mamont::service::mount::{ExportEntryWrapper, MountService};
use nfs_mamont::service::nlm::NlmService;
use nfs_mamont::vfs::file::{BackendId, Path};
use nfs_mamont::{handle_forever, BackendRegistry, Impl, ServerContext};

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

        let allocator = Arc::new(Impl::new(config.buf_size, config.buf_count));
        let context = ServerContext::new(allocator, config.concurrency);
        let backend = Arc::new(MockVfs::new(config.vfs));
        let backend_id = context
            .add_backend(Arc::clone(&backend))
            .ok_or_else(|| io::Error::other("no free backend slot"))?;
        let backends = context.backends();

        let export = ExportEntryWrapper {
            export: ExportEntry { directory: Path::new(config.export)?, names: Vec::new() },
            root_handle: MockVfs::root_handle(backend_id),
        };
        let mount_service = Arc::new(MountService::with_exports(vec![export]));
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
