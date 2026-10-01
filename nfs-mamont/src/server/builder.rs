use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use crate::backend::BackendRegistry;
use crate::service::mount::MountService;
use crate::service::nlm::NlmService;

use super::{Server, DEFAULT_SHUTDOWN_TIMEOUT};

/// Address the server listens on unless [`ServerBuilder::bind`] is called:
/// the well-known NFS port on all IPv4 interfaces.
const DEFAULT_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 2049);

/// Builder of a [`Server`], created by [`Server::builder`].
///
/// Every setting except the allocator has a default:
///
/// | Setting                                      | Default                          |
/// |----------------------------------------------|----------------------------------|
/// | [`bind`](Self::bind)                         | `0.0.0.0:2049`                   |
/// | [`backends`](Self::backends)                 | empty registry                   |
/// | [`mount`](Self::mount)                       | [`MountService`] without exports |
/// | [`nlm`](Self::nlm)                           | [`NlmService`]                   |
/// | [`rpcbind`](Self::rpcbind)                   | enabled                          |
/// | [`shutdown_timeout`](Self::shutdown_timeout) | 30 seconds                       |
pub struct ServerBuilder<A, V, M = MountService, N = NlmService> {
    addr: SocketAddr,
    rpcbind: bool,
    shutdown_timeout: Duration,
    allocator: Arc<A>,
    backends: BackendRegistry<V>,
    mount_service: Arc<M>,
    nlm_service: Arc<N>,
}

impl<A, V> ServerBuilder<A, V> {
    /// Creates a builder with default settings and the given buffer allocator.
    pub(super) fn new(allocator: Arc<A>) -> Self {
        Self {
            addr: DEFAULT_ADDR,
            rpcbind: true,
            shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
            allocator,
            backends: BackendRegistry::new(),
            mount_service: Arc::new(MountService::with_exports(Vec::new())),
            nlm_service: Arc::new(NlmService::new()),
        }
    }
}

impl<A, V, M, N> ServerBuilder<A, V, M, N> {
    /// Sets the address to listen on.
    ///
    /// The address is bound anew on every [`Server::start`]; port `0` picks a free
    /// port, which [`Server::start`] returns.
    pub fn bind(mut self, addr: SocketAddr) -> Self {
        self.addr = addr;
        self
    }

    /// Sets the registry of backends serving NFS requests.
    ///
    /// The registry is shared, not copied: backends added to or removed from
    /// `backends` later take effect on the running server as well.
    pub fn backends(mut self, backends: BackendRegistry<V>) -> Self {
        self.backends = backends;
        self
    }

    /// Sets the service answering MOUNT requests.
    pub fn mount<M2>(self, mount_service: Arc<M2>) -> ServerBuilder<A, V, M2, N> {
        ServerBuilder {
            addr: self.addr,
            rpcbind: self.rpcbind,
            shutdown_timeout: self.shutdown_timeout,
            allocator: self.allocator,
            backends: self.backends,
            mount_service,
            nlm_service: self.nlm_service,
        }
    }

    /// Sets the service answering NLM requests.
    pub fn nlm<N2>(self, nlm_service: Arc<N2>) -> ServerBuilder<A, V, M, N2> {
        ServerBuilder {
            addr: self.addr,
            rpcbind: self.rpcbind,
            shutdown_timeout: self.shutdown_timeout,
            allocator: self.allocator,
            backends: self.backends,
            mount_service: self.mount_service,
            nlm_service,
        }
    }

    /// Sets whether to publish the NFS, MOUNT and NLM services with the local rpcbind
    /// on start and withdraw them on stop.
    ///
    /// Registration is best-effort: when it fails the server still runs, but clients
    /// have to pass `port=`/`mountport=` mount options.
    pub fn rpcbind(mut self, enabled: bool) -> Self {
        self.rpcbind = enabled;
        self
    }

    /// Sets how long [`Server::stop`] waits for in-flight requests before aborting
    /// the remaining tasks. [`Duration::MAX`] waits indefinitely.
    pub fn shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }

    /// Builds a stopped [`Server`]; call [`Server::start`] to run it.
    pub fn build(self) -> Server<A, V, M, N> {
        Server {
            addr: self.addr,
            rpcbind: self.rpcbind,
            shutdown_timeout: self.shutdown_timeout,
            allocator: self.allocator,
            backends: self.backends,
            mount_service: self.mount_service,
            nlm_service: self.nlm_service,
            running: None,
        }
    }
}
