use std::collections::BTreeMap;
use std::io;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::string::FromUtf8Error;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use num_derive::{FromPrimitive, ToPrimitive};
use tokio::sync::RwLock;
use tracing::{info_span, Span};

use crate::allocator::Slice;
use crate::mount;
use crate::parser::Arguments;
use crate::vfs;
use crate::vfs::file;

pub const RPC_VERSION: u32 = 2;

pub const MAX_AUTH_SIZE: usize = 400;

#[derive(ToPrimitive, FromPrimitive)]
pub enum AcceptStat {
    Success = 0,
    ProgUnavail = 1,
    ProgMismatch = 2,
    ProcUnavail = 3,
    GarbageArgs = 4,
    SystemErr = 5,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ToPrimitive, FromPrimitive)]
pub enum AuthStat {
    Ok = 0,
    BadCred = 1,
    RejectedCred = 2,
    BadVerf = 3,
    RejectedVerf = 4,
    TooWeak = 5,
    InvalidResp = 6,
    Failed = 7,
    KerbGeneric = 8,
    TimeExpire = 9,
    TktFile = 10,
    Decode = 11,
    NetAddr = 12,
    RpcSecGssCredProblem = 13,
    RpcSecGssCtxProblem = 14,
}

#[derive(ToPrimitive, FromPrimitive)]
pub enum RpcBody {
    Call = 0,
    Reply = 1,
}

pub enum ReplyBody {
    MsgAccepted = 0,
    MsgDenied = 1,
}

/// Authentication flavors.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ToPrimitive, FromPrimitive)]
pub enum AuthFlavor {
    None = 0,
    Sys = 1,
    Short = 2,
    Dh = 3,
    RpcSecGss = 6,
}

#[derive(Debug, Clone)]
pub struct OpaqueAuth {
    pub flavor: AuthFlavor,
    pub body: Vec<u8>,
}

/// Point-in-time gauge snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GaugeSnapshot {
    /// Current observed value.
    pub current: usize,
    /// Highest observed value.
    pub peak: usize,
}

/// Aggregated latency statistics measured in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencySnapshot {
    /// Number of recorded samples.
    pub samples: u64,
    /// Sum of all observed latencies.
    pub total_micros: u64,
    /// Integer average latency.
    pub average_micros: u64,
    /// Maximum observed latency.
    pub max_micros: u64,
}

/// Snapshot of exported server metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerMetricsSnapshot {
    /// Requests successfully accepted into the command queue.
    pub requests_received: u64,
    /// Requests rejected during parsing or reply serialization.
    pub requests_rejected: u64,
    /// Requests dispatched to protocol handlers.
    pub requests_dispatched: u64,
    /// Replies successfully written to the client socket.
    pub replies_sent: u64,
    /// Replies dropped before reaching the socket.
    pub reply_failures: u64,
    /// Command queue occupancy.
    pub command_queue_depth: GaugeSnapshot,
    /// Result queue occupancy.
    pub result_queue_depth: GaugeSnapshot,
    /// In-flight request occupancy.
    pub in_flight_requests: GaugeSnapshot,
    /// Time spent waiting before dispatch.
    pub queue_wait: LatencySnapshot,
    /// Time spent inside dispatch and serialization.
    pub dispatch: LatencySnapshot,
    /// End-to-end time from request acceptance to successful write.
    pub total_latency: LatencySnapshot,
    /// Time spent between dispatch completion and socket write completion.
    pub dispatch_to_write: LatencySnapshot,
}

/// Shared server metrics backend backed by atomics.
#[derive(Debug, Default)]
pub struct ServerMetrics {
    requests_received: AtomicU64,
    requests_rejected: AtomicU64,
    requests_dispatched: AtomicU64,
    replies_sent: AtomicU64,
    reply_failures: AtomicU64,
    command_queue_current: AtomicUsize,
    command_queue_peak: AtomicUsize,
    result_queue_current: AtomicUsize,
    result_queue_peak: AtomicUsize,
    in_flight_current: AtomicUsize,
    in_flight_peak: AtomicUsize,
    queue_wait_samples: AtomicU64,
    queue_wait_total_micros: AtomicU64,
    queue_wait_max_micros: AtomicU64,
    dispatch_samples: AtomicU64,
    dispatch_total_micros: AtomicU64,
    dispatch_max_micros: AtomicU64,
    total_latency_samples: AtomicU64,
    total_latency_total_micros: AtomicU64,
    total_latency_max_micros: AtomicU64,
    dispatch_to_write_samples: AtomicU64,
    dispatch_to_write_total_micros: AtomicU64,
    dispatch_to_write_max_micros: AtomicU64,
}

impl ServerMetrics {
    /// Creates a new metrics backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an atomic snapshot of all exported metrics.
    pub fn snapshot(&self) -> ServerMetricsSnapshot {
        ServerMetricsSnapshot {
            requests_received: self.requests_received.load(Ordering::Relaxed),
            requests_rejected: self.requests_rejected.load(Ordering::Relaxed),
            requests_dispatched: self.requests_dispatched.load(Ordering::Relaxed),
            replies_sent: self.replies_sent.load(Ordering::Relaxed),
            reply_failures: self.reply_failures.load(Ordering::Relaxed),
            command_queue_depth: GaugeSnapshot {
                current: self.command_queue_current.load(Ordering::Relaxed),
                peak: self.command_queue_peak.load(Ordering::Relaxed),
            },
            result_queue_depth: GaugeSnapshot {
                current: self.result_queue_current.load(Ordering::Relaxed),
                peak: self.result_queue_peak.load(Ordering::Relaxed),
            },
            in_flight_requests: GaugeSnapshot {
                current: self.in_flight_current.load(Ordering::Relaxed),
                peak: self.in_flight_peak.load(Ordering::Relaxed),
            },
            queue_wait: Self::latency_snapshot(
                &self.queue_wait_samples,
                &self.queue_wait_total_micros,
                &self.queue_wait_max_micros,
            ),
            dispatch: Self::latency_snapshot(
                &self.dispatch_samples,
                &self.dispatch_total_micros,
                &self.dispatch_max_micros,
            ),
            total_latency: Self::latency_snapshot(
                &self.total_latency_samples,
                &self.total_latency_total_micros,
                &self.total_latency_max_micros,
            ),
            dispatch_to_write: Self::latency_snapshot(
                &self.dispatch_to_write_samples,
                &self.dispatch_to_write_total_micros,
                &self.dispatch_to_write_max_micros,
            ),
        }
    }

    fn latency_snapshot(
        samples: &AtomicU64,
        total_micros: &AtomicU64,
        max_micros: &AtomicU64,
    ) -> LatencySnapshot {
        let samples = samples.load(Ordering::Relaxed);
        let total_micros = total_micros.load(Ordering::Relaxed);
        LatencySnapshot {
            samples,
            total_micros,
            average_micros: if samples == 0 { 0 } else { total_micros / samples },
            max_micros: max_micros.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn record_request_received(&self, command_queue_depth: usize) {
        self.requests_received.fetch_add(1, Ordering::Relaxed);
        self.observe_command_queue_depth(command_queue_depth);
    }

    pub(crate) fn record_request_rejected(&self) {
        self.requests_rejected.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn observe_command_queue_depth(&self, depth: usize) {
        self.command_queue_current.store(depth, Ordering::Relaxed);
        update_peak_usize(&self.command_queue_peak, depth);
    }

    pub(crate) fn observe_result_queue_depth(&self, depth: usize) {
        self.result_queue_current.store(depth, Ordering::Relaxed);
        update_peak_usize(&self.result_queue_peak, depth);
    }

    pub(crate) fn start_in_flight_request(&self) {
        let in_flight = self.in_flight_current.fetch_add(1, Ordering::Relaxed) + 1;
        update_peak_usize(&self.in_flight_peak, in_flight);
    }

    pub(crate) fn finish_in_flight_request(&self) {
        self.in_flight_current.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn record_dispatch(&self, queue_wait_micros: u64, dispatch_micros: u64) {
        self.requests_dispatched.fetch_add(1, Ordering::Relaxed);
        record_latency(
            &self.queue_wait_samples,
            &self.queue_wait_total_micros,
            &self.queue_wait_max_micros,
            queue_wait_micros,
        );
        record_latency(
            &self.dispatch_samples,
            &self.dispatch_total_micros,
            &self.dispatch_max_micros,
            dispatch_micros,
        );
    }

    pub(crate) fn record_reply_sent(
        &self,
        total_latency_micros: u64,
        dispatch_to_write_micros: u64,
    ) {
        self.replies_sent.fetch_add(1, Ordering::Relaxed);
        record_latency(
            &self.total_latency_samples,
            &self.total_latency_total_micros,
            &self.total_latency_max_micros,
            total_latency_micros,
        );
        record_latency(
            &self.dispatch_to_write_samples,
            &self.dispatch_to_write_total_micros,
            &self.dispatch_to_write_max_micros,
            dispatch_to_write_micros,
        );
    }

    pub(crate) fn record_reply_failure(&self) {
        self.reply_failures.fetch_add(1, Ordering::Relaxed);
    }
}

fn record_latency(
    samples: &AtomicU64,
    total_micros: &AtomicU64,
    max_micros: &AtomicU64,
    value: u64,
) {
    samples.fetch_add(1, Ordering::Relaxed);
    total_micros.fetch_add(value, Ordering::Relaxed);
    update_peak_u64(max_micros, value);
}

fn update_peak_usize(peak: &AtomicUsize, value: usize) {
    let mut current = peak.load(Ordering::Relaxed);
    while value > current {
        match peak.compare_exchange(current, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

fn update_peak_u64(peak: &AtomicU64, value: u64) {
    let mut current = peak.load(Ordering::Relaxed);
    while value > current {
        match peak.compare_exchange(current, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

pub type SharedVfs = Arc<dyn vfs::Vfs + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub struct ServerSettings {
    read_buffer_size: NonZeroUsize,
    allocator_buffer_size: NonZeroUsize,
    allocator_buffer_count: NonZeroUsize,
    command_queue_size: NonZeroUsize,
    result_queue_size: NonZeroUsize,
    max_in_flight_requests: NonZeroUsize,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            read_buffer_size: default_non_zero_usize(4096),
            allocator_buffer_size: default_non_zero_usize(4096),
            allocator_buffer_count: default_non_zero_usize(16),
            command_queue_size: default_non_zero_usize(64),
            result_queue_size: default_non_zero_usize(64),
            max_in_flight_requests: default_non_zero_usize(8),
        }
    }
}

impl ServerSettings {
    /// Creates a settings value with defaults suitable for a small server.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the read buffer size in bytes.
    pub fn read_buffer_size(&self) -> NonZeroUsize {
        self.read_buffer_size
    }

    /// Sets the read buffer size in bytes.
    pub fn with_read_buffer_size(mut self, read_buffer_size: NonZeroUsize) -> Self {
        self.read_buffer_size = read_buffer_size;
        self
    }

    /// Returns the allocator buffer size in bytes.
    pub fn allocator_buffer_size(&self) -> NonZeroUsize {
        self.allocator_buffer_size
    }

    /// Sets the allocator buffer size in bytes.
    pub fn with_allocator_buffer_size(mut self, allocator_buffer_size: NonZeroUsize) -> Self {
        self.allocator_buffer_size = allocator_buffer_size;
        self
    }

    /// Returns the number of allocator buffers.
    pub fn allocator_buffer_count(&self) -> NonZeroUsize {
        self.allocator_buffer_count
    }

    /// Sets the number of allocator buffers.
    pub fn with_allocator_buffer_count(mut self, allocator_buffer_count: NonZeroUsize) -> Self {
        self.allocator_buffer_count = allocator_buffer_count;
        self
    }

    /// Returns the maximum number of queued RPC commands per connection.
    pub fn command_queue_size(&self) -> NonZeroUsize {
        self.command_queue_size
    }

    /// Sets the maximum number of queued RPC commands per connection.
    pub fn with_command_queue_size(mut self, command_queue_size: NonZeroUsize) -> Self {
        self.command_queue_size = command_queue_size;
        self
    }

    /// Returns the maximum number of queued RPC replies per connection.
    pub fn result_queue_size(&self) -> NonZeroUsize {
        self.result_queue_size
    }

    /// Sets the maximum number of queued RPC replies per connection.
    pub fn with_result_queue_size(mut self, result_queue_size: NonZeroUsize) -> Self {
        self.result_queue_size = result_queue_size;
        self
    }

    /// Returns the maximum number of concurrently processed RPC requests per connection.
    pub fn max_in_flight_requests(&self) -> NonZeroUsize {
        self.max_in_flight_requests
    }

    /// Sets the maximum number of concurrently processed RPC requests per connection.
    pub fn with_max_in_flight_requests(mut self, max_in_flight_requests: NonZeroUsize) -> Self {
        self.max_in_flight_requests = max_in_flight_requests;
        self
    }
}

fn default_non_zero_usize(value: usize) -> NonZeroUsize {
    match NonZeroUsize::new(value) {
        Some(value) => value,
        None => unreachable!("default server setting must be non-zero"),
    }
}

#[derive(Debug, Clone)]
pub struct ServerExport {
    directory: file::Path,
    allowed_hosts: Vec<String>,
}

#[derive(Clone)]
pub struct ServerContext {
    settings: ServerSettings,
    backend: Option<SharedVfs>,
    exports: Arc<RwLock<ExportRegistry>>,
    mounts: Arc<RwLock<MountRegistry>>,
    metrics: Arc<ServerMetrics>,
}

impl Default for ServerContext {
    fn default() -> Self {
        Self {
            settings: ServerSettings::default(),
            backend: None,
            exports: Arc::new(RwLock::new(ExportRegistry::default())),
            mounts: Arc::new(RwLock::new(MountRegistry::default())),
            metrics: Arc::new(ServerMetrics::default()),
        }
    }
}

impl ServerContext {
    /// Creates a new context with the provided settings and no backend.
    pub fn with_settings(settings: ServerSettings) -> Self {
        Self { settings, ..Self::default() }
    }

    /// Creates a new context with the provided backend.
    pub fn with_backend(backend: SharedVfs) -> Self {
        Self { backend: Some(backend), ..Self::default() }
    }

    /// Creates a new context with the provided backend and settings.
    pub fn with_backend_and_settings(backend: SharedVfs, settings: ServerSettings) -> Self {
        Self { settings, backend: Some(backend), ..Self::default() }
    }

    /// Returns the server settings.
    pub fn settings(&self) -> &ServerSettings {
        &self.settings
    }

    /// Returns the configured backend, if any.
    pub fn backend(&self) -> Option<&SharedVfs> {
        self.backend.as_ref()
    }

    /// Returns the shared metrics backend.
    pub fn metrics(&self) -> Arc<ServerMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Registers an exported directory.
    pub async fn add_export(&self, export: ServerExport) {
        self.exports.write().await.insert(export);
    }

    /// Returns a snapshot of configured exports.
    pub async fn exports(&self) -> Vec<ServerExport> {
        self.exports.read().await.snapshot()
    }

    /// Finds a configured export by its directory path.
    pub async fn find_export(&self, directory: &file::Path) -> Option<ServerExport> {
        self.exports.read().await.find(directory).cloned()
    }

    /// Records a mounted directory for a client.
    pub async fn record_mount(&self, client_addr: String, directory: file::Path) {
        self.mounts.write().await.record(client_addr, directory);
    }

    /// Returns a snapshot of active mounts.
    pub async fn mount_entries(&self) -> Vec<(String, file::Path)> {
        self.mounts.read().await.snapshot()
    }

    /// Removes a single mounted directory for a client.
    pub async fn remove_mount(&self, client_addr: &str, directory: &file::Path) {
        self.mounts.write().await.remove(client_addr, directory);
    }

    /// Removes all mounted directories associated with a client.
    pub async fn remove_mounts_by_client(&self, client_addr: &str) {
        self.mounts.write().await.remove_client(client_addr);
    }
}

#[cfg(test)]
mod tests {
    use super::{ServerMetrics, ServerSettings};

    #[test]
    fn server_settings_allow_queue_configuration() {
        let settings = ServerSettings::new()
            .with_command_queue_size(std::num::NonZeroUsize::new(8).expect("non-zero literal"))
            .with_result_queue_size(std::num::NonZeroUsize::new(16).expect("non-zero literal"))
            .with_max_in_flight_requests(std::num::NonZeroUsize::new(4).expect("non-zero literal"));

        assert_eq!(settings.command_queue_size().get(), 8);
        assert_eq!(settings.result_queue_size().get(), 16);
        assert_eq!(settings.max_in_flight_requests().get(), 4);
    }

    #[test]
    fn server_metrics_snapshot_exposes_aggregates() {
        let metrics = ServerMetrics::new();

        metrics.record_request_received(3);
        metrics.record_request_rejected();
        metrics.observe_result_queue_depth(2);
        metrics.start_in_flight_request();
        metrics.record_dispatch(10, 20);
        metrics.record_reply_sent(40, 15);
        metrics.record_reply_failure();
        metrics.finish_in_flight_request();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.requests_received, 1);
        assert_eq!(snapshot.requests_rejected, 1);
        assert_eq!(snapshot.requests_dispatched, 1);
        assert_eq!(snapshot.replies_sent, 1);
        assert_eq!(snapshot.reply_failures, 1);
        assert_eq!(snapshot.command_queue_depth.current, 3);
        assert_eq!(snapshot.command_queue_depth.peak, 3);
        assert_eq!(snapshot.result_queue_depth.current, 2);
        assert_eq!(snapshot.result_queue_depth.peak, 2);
        assert_eq!(snapshot.in_flight_requests.current, 0);
        assert_eq!(snapshot.in_flight_requests.peak, 1);
        assert_eq!(snapshot.queue_wait.average_micros, 10);
        assert_eq!(snapshot.dispatch.average_micros, 20);
        assert_eq!(snapshot.total_latency.average_micros, 40);
        assert_eq!(snapshot.dispatch_to_write.average_micros, 15);
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionContext {
    local_addr: Option<SocketAddr>,
    client_addr: Option<SocketAddr>,
    auth: Option<AuthFlavor>,
}

impl ConnectionContext {
    /// Creates a new connection context.
    pub fn new(local_addr: Option<SocketAddr>, client_addr: Option<SocketAddr>) -> Self {
        Self { local_addr, client_addr, auth: None }
    }

    /// Returns the local socket address.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// Returns the client socket address.
    pub fn client_addr(&self) -> Option<SocketAddr> {
        self.client_addr
    }

    /// Returns the negotiated authentication flavor.
    pub fn auth(&self) -> Option<AuthFlavor> {
        self.auth
    }

    fn with_auth(mut self, auth: AuthFlavor) -> Self {
        self.auth = Some(auth);
        self
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RpcCallHeader {
    pub xid: u32,
    pub program: u32,
    pub version: u32,
    pub procedure: u32,
    pub auth_flavor: AuthFlavor,
}

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub connection: ConnectionContext,
    pub header: RpcCallHeader,
    pub span: Span,
    pub received_at: Instant,
}

pub struct ParsedRpcCall {
    pub header: RpcCallHeader,
    pub arguments: Box<Arguments>,
}

impl ParsedRpcCall {
    pub fn with_connection(self, connection: ConnectionContext) -> RpcCommand {
        let auth = self.header.auth_flavor;
        let connection = connection.with_auth(auth);
        let span = request_span(&connection, self.header);
        RpcCommand {
            context: RequestContext {
                connection,
                header: self.header,
                span,
                received_at: Instant::now(),
            },
            arguments: self.arguments,
        }
    }
}

impl ServerExport {
    /// Creates a new export definition.
    pub fn new(directory: file::Path, allowed_hosts: Vec<String>) -> Self {
        Self { directory, allowed_hosts }
    }

    /// Returns the exported directory.
    pub fn directory(&self) -> &file::Path {
        &self.directory
    }

    /// Returns the allowed host patterns.
    pub fn allowed_hosts(&self) -> &[String] {
        &self.allowed_hosts
    }
}

pub struct RpcCommand {
    pub context: RequestContext,
    pub arguments: Box<Arguments>,
}

pub struct ReplyEnvelope {
    pub result: CommandResult,
    pub span: Span,
    pub received_at: Instant,
    pub dispatched_at: Option<Instant>,
}

impl ReplyEnvelope {
    pub fn new(
        result: CommandResult,
        span: Span,
        received_at: Instant,
        dispatched_at: Option<Instant>,
    ) -> Self {
        Self { result, span, received_at, dispatched_at }
    }
}

pub enum ReplyPayload {
    Buffer(Vec<u8>),
    Read { header: Vec<u8>, data: Slice, padding: usize },
}

pub struct RpcReply {
    pub xid: u32,
    pub payload: ReplyPayload,
}

impl RpcReply {
    pub fn new(xid: u32, payload: ReplyPayload) -> Self {
        Self { xid, payload }
    }
}

pub type CommandResult = io::Result<RpcReply>;

#[derive(Default)]
struct ExportRegistry {
    by_directory: BTreeMap<PathBuf, ServerExport>,
}

impl ExportRegistry {
    fn insert(&mut self, export: ServerExport) {
        self.by_directory.insert(export.directory().as_path().to_path_buf(), export);
    }

    fn find(&self, directory: &file::Path) -> Option<&ServerExport> {
        self.by_directory.get(directory.as_path())
    }

    fn snapshot(&self) -> Vec<ServerExport> {
        self.by_directory.values().cloned().collect()
    }

    fn export_entries(&self) -> Vec<mount::ExportEntry> {
        self.by_directory
            .values()
            .map(|export| mount::ExportEntry {
                directory: export.directory().clone(),
                names: export.allowed_hosts().to_vec(),
            })
            .collect()
    }
}

#[derive(Default)]
struct MountRegistry {
    by_client: BTreeMap<String, BTreeMap<PathBuf, file::Path>>,
}

impl MountRegistry {
    fn record(&mut self, client_addr: String, directory: file::Path) {
        self.by_client
            .entry(client_addr)
            .or_default()
            .insert(directory.as_path().to_path_buf(), directory);
    }

    fn snapshot(&self) -> Vec<(String, file::Path)> {
        self.by_client
            .iter()
            .flat_map(|(client_addr, directories)| {
                directories.values().cloned().map(|directory| (client_addr.clone(), directory))
            })
            .collect()
    }

    fn mount_entries(&self) -> Vec<mount::MountEntry> {
        self.by_client
            .iter()
            .flat_map(|(client_addr, directories)| {
                directories
                    .values()
                    .cloned()
                    .map(|directory| mount::MountEntry { hostname: client_addr.clone(), directory })
            })
            .collect()
    }

    fn remove(&mut self, client_addr: &str, directory: &file::Path) {
        let should_remove_client = match self.by_client.get_mut(client_addr) {
            Some(directories) => {
                directories.remove(directory.as_path());
                directories.is_empty()
            }
            None => false,
        };

        if should_remove_client {
            self.by_client.remove(client_addr);
        }
    }

    fn remove_client(&mut self, client_addr: &str) {
        self.by_client.remove(client_addr);
    }
}

pub fn request_span(connection: &ConnectionContext, header: RpcCallHeader) -> Span {
    info_span!(
        "rpc_request",
        xid = header.xid,
        program = header.program,
        version = header.version,
        procedure = header.procedure,
        auth = ?header.auth_flavor,
        peer = %socket_addr_label(connection.client_addr()),
        local = %socket_addr_label(connection.local_addr()),
    )
}

pub fn rejected_request_span(connection: &ConnectionContext, xid: u32) -> Span {
    info_span!(
        "rpc_request",
        xid,
        peer = %socket_addr_label(connection.client_addr()),
        local = %socket_addr_label(connection.local_addr()),
    )
}

fn socket_addr_label(socket_addr: Option<SocketAddr>) -> String {
    socket_addr.map(|addr| addr.to_string()).unwrap_or_else(|| "unknown".to_string())
}

impl ServerContext {
    /// Returns active mounts already converted into mount protocol response entries.
    pub async fn mount_dump_entries(&self) -> Vec<mount::MountEntry> {
        self.mounts.read().await.mount_entries()
    }

    /// Returns exports already converted into mount protocol response entries.
    pub async fn export_entries(&self) -> Vec<mount::ExportEntry> {
        self.exports.read().await.export_entries()
    }
}

pub enum RejectedReply {
    RpcMismatch = 0,
    AuthError = 1,
}

/// Represents a mismatch in program/protocol versions.
/// Returns highest and lowest versions of available versions of requested program
#[derive(Debug)]
pub struct VersionMismatch {
    pub low: u32,
    pub high: u32,
}

/// Errors that can occur during parsing.
#[derive(Debug)]
pub enum Error {
    /// The maximum element limit was exceeded.
    MaxElemLimit,
    /// An I/O error occurred.
    IO(io::Error),
    /// An enum discriminant mismatch occurred.
    EnumDiscMismatch,
    /// An incorrect string was encountered during UTF-8 conversion.
    IncorrectString(FromUtf8Error),
    /// Incorrect padding was found.
    IncorrectPadding,
    /// An impossible type cast was attempted.
    ImpossibleTypeCast,
    /// A bad file handle was encountered.
    BadFileHandle,
    /// A message type mismatch occurred.
    MessageTypeMismatch,
    /// An RPC version mismatch occurred.
    RpcVersionMismatch(VersionMismatch),
    /// An authentication error occurred.
    AuthError(AuthStat),
    /// A program mismatch occurred.
    ProgramMismatch,
    /// A procedure mismatch occurred.
    ProcedureMismatch,
    /// A program version mismatch occurred.
    ProgramVersionMismatch(VersionMismatch),
}
