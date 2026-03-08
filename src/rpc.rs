use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::string::FromUtf8Error;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
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

/// Pool of reusable reply buffers for non-streamed RPC replies.
#[derive(Debug, Default)]
pub struct ReplyBufferPool {
    buffers: Mutex<Vec<Vec<u8>>>,
}

impl ReplyBufferPool {
    /// Creates a new empty reply buffer pool.
    pub fn new() -> Self {
        Self::default()
    }

    /// Acquires a reusable reply buffer with at least the requested capacity.
    pub fn acquire(self: &Arc<Self>, capacity: usize) -> OwnedReplyBuffer {
        let mut data = self
            .buffers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop()
            .unwrap_or_default();
        data.clear();
        if data.capacity() < capacity {
            data.reserve(capacity - data.capacity());
        }
        OwnedReplyBuffer { data, pool: Arc::clone(self) }
    }

    fn release(&self, mut data: Vec<u8>) {
        data.clear();
        self.buffers.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(data);
    }
}

/// Reply buffer that automatically returns its storage back to the pool on drop.
#[derive(Debug)]
pub struct OwnedReplyBuffer {
    data: Vec<u8>,
    pool: Arc<ReplyBufferPool>,
}

impl OwnedReplyBuffer {
    /// Returns the buffered reply bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Returns the reply length.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns whether the reply buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns mutable access to the underlying reply buffer.
    pub fn as_mut_vec(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }
}

impl Drop for OwnedReplyBuffer {
    fn drop(&mut self) {
        let data = std::mem::take(&mut self.data);
        self.pool.release(data);
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProcedureLabels {
    program: &'static str,
    version: &'static str,
    procedure: &'static str,
}

impl ProcedureLabels {
    const fn new(program: &'static str, version: &'static str, procedure: &'static str) -> Self {
        Self { program, version, procedure }
    }
}

#[derive(Debug)]
struct ProcedureMetricsEntry {
    labels: ProcedureLabels,
    requests_received: AtomicU64,
    requests_dispatched: AtomicU64,
    replies_sent: AtomicU64,
    reply_failures: AtomicU64,
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

impl ProcedureMetricsEntry {
    fn new(labels: ProcedureLabels) -> Self {
        Self {
            labels,
            requests_received: AtomicU64::new(0),
            requests_dispatched: AtomicU64::new(0),
            replies_sent: AtomicU64::new(0),
            reply_failures: AtomicU64::new(0),
            queue_wait_samples: AtomicU64::new(0),
            queue_wait_total_micros: AtomicU64::new(0),
            queue_wait_max_micros: AtomicU64::new(0),
            dispatch_samples: AtomicU64::new(0),
            dispatch_total_micros: AtomicU64::new(0),
            dispatch_max_micros: AtomicU64::new(0),
            total_latency_samples: AtomicU64::new(0),
            total_latency_total_micros: AtomicU64::new(0),
            total_latency_max_micros: AtomicU64::new(0),
            dispatch_to_write_samples: AtomicU64::new(0),
            dispatch_to_write_total_micros: AtomicU64::new(0),
            dispatch_to_write_max_micros: AtomicU64::new(0),
        }
    }

    fn snapshot(&self) -> ProcedureMetricsSnapshot {
        ProcedureMetricsSnapshot {
            labels: self.labels,
            requests_received: self.requests_received.load(Ordering::Relaxed),
            requests_dispatched: self.requests_dispatched.load(Ordering::Relaxed),
            replies_sent: self.replies_sent.load(Ordering::Relaxed),
            reply_failures: self.reply_failures.load(Ordering::Relaxed),
            queue_wait: ServerMetrics::latency_snapshot(
                &self.queue_wait_samples,
                &self.queue_wait_total_micros,
                &self.queue_wait_max_micros,
            ),
            dispatch: ServerMetrics::latency_snapshot(
                &self.dispatch_samples,
                &self.dispatch_total_micros,
                &self.dispatch_max_micros,
            ),
            total_latency: ServerMetrics::latency_snapshot(
                &self.total_latency_samples,
                &self.total_latency_total_micros,
                &self.total_latency_max_micros,
            ),
            dispatch_to_write: ServerMetrics::latency_snapshot(
                &self.dispatch_to_write_samples,
                &self.dispatch_to_write_total_micros,
                &self.dispatch_to_write_max_micros,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProcedureMetricsSnapshot {
    labels: ProcedureLabels,
    requests_received: u64,
    requests_dispatched: u64,
    replies_sent: u64,
    reply_failures: u64,
    queue_wait: LatencySnapshot,
    dispatch: LatencySnapshot,
    total_latency: LatencySnapshot,
    dispatch_to_write: LatencySnapshot,
}

const PROCEDURE_LABELS: [ProcedureLabels; 28] = [
    ProcedureLabels::new("nfs", "3", "null"),
    ProcedureLabels::new("nfs", "3", "getattr"),
    ProcedureLabels::new("nfs", "3", "setattr"),
    ProcedureLabels::new("nfs", "3", "lookup"),
    ProcedureLabels::new("nfs", "3", "access"),
    ProcedureLabels::new("nfs", "3", "readlink"),
    ProcedureLabels::new("nfs", "3", "read"),
    ProcedureLabels::new("nfs", "3", "write"),
    ProcedureLabels::new("nfs", "3", "create"),
    ProcedureLabels::new("nfs", "3", "mkdir"),
    ProcedureLabels::new("nfs", "3", "symlink"),
    ProcedureLabels::new("nfs", "3", "mknod"),
    ProcedureLabels::new("nfs", "3", "remove"),
    ProcedureLabels::new("nfs", "3", "rmdir"),
    ProcedureLabels::new("nfs", "3", "rename"),
    ProcedureLabels::new("nfs", "3", "link"),
    ProcedureLabels::new("nfs", "3", "readdir"),
    ProcedureLabels::new("nfs", "3", "readdirplus"),
    ProcedureLabels::new("nfs", "3", "fsstat"),
    ProcedureLabels::new("nfs", "3", "fsinfo"),
    ProcedureLabels::new("nfs", "3", "pathconf"),
    ProcedureLabels::new("nfs", "3", "commit"),
    ProcedureLabels::new("mount", "3", "null"),
    ProcedureLabels::new("mount", "3", "mnt"),
    ProcedureLabels::new("mount", "3", "dump"),
    ProcedureLabels::new("mount", "3", "umnt"),
    ProcedureLabels::new("mount", "3", "umntall"),
    ProcedureLabels::new("mount", "3", "export"),
];

fn procedure_metric_index(header: RpcCallHeader) -> Option<usize> {
    match (header.program, header.version, header.procedure) {
        (crate::nfsv3::NFS_PROGRAM, crate::nfsv3::NFS_VERSION, procedure @ 0..=21) => {
            Some(procedure as usize)
        }
        (crate::mount::MOUNT_PROGRAM, crate::mount::MOUNT_VERSION, procedure @ 0..=5) => {
            Some(crate::nfsv3::COMMIT as usize + 1 + procedure as usize)
        }
        _ => None,
    }
}

pub(crate) fn procedure_labels(header: RpcCallHeader) -> ProcedureLabels {
    procedure_metric_index(header)
        .map(|index| PROCEDURE_LABELS[index])
        .unwrap_or_else(|| ProcedureLabels::new("unknown", "unknown", "unknown"))
}

/// Shared server metrics backend backed by atomics.
#[derive(Debug)]
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
    procedure_metrics: Box<[ProcedureMetricsEntry]>,
}

impl ServerMetrics {
    /// Creates a new metrics backend.
    pub fn new() -> Self {
        Self {
            requests_received: AtomicU64::new(0),
            requests_rejected: AtomicU64::new(0),
            requests_dispatched: AtomicU64::new(0),
            replies_sent: AtomicU64::new(0),
            reply_failures: AtomicU64::new(0),
            command_queue_current: AtomicUsize::new(0),
            command_queue_peak: AtomicUsize::new(0),
            result_queue_current: AtomicUsize::new(0),
            result_queue_peak: AtomicUsize::new(0),
            in_flight_current: AtomicUsize::new(0),
            in_flight_peak: AtomicUsize::new(0),
            queue_wait_samples: AtomicU64::new(0),
            queue_wait_total_micros: AtomicU64::new(0),
            queue_wait_max_micros: AtomicU64::new(0),
            dispatch_samples: AtomicU64::new(0),
            dispatch_total_micros: AtomicU64::new(0),
            dispatch_max_micros: AtomicU64::new(0),
            total_latency_samples: AtomicU64::new(0),
            total_latency_total_micros: AtomicU64::new(0),
            total_latency_max_micros: AtomicU64::new(0),
            dispatch_to_write_samples: AtomicU64::new(0),
            dispatch_to_write_total_micros: AtomicU64::new(0),
            dispatch_to_write_max_micros: AtomicU64::new(0),
            procedure_metrics: PROCEDURE_LABELS
                .into_iter()
                .map(ProcedureMetricsEntry::new)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
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

    /// Encodes the current metrics snapshot in Prometheus text format.
    pub fn encode_prometheus(&self) -> String {
        encode_metrics(&self.snapshot(), &self.procedure_snapshots(), false)
    }

    /// Encodes the current metrics snapshot in OpenMetrics text format.
    pub fn encode_openmetrics(&self) -> String {
        encode_metrics(&self.snapshot(), &self.procedure_snapshots(), true)
    }

    fn procedure_snapshots(&self) -> Vec<ProcedureMetricsSnapshot> {
        self.procedure_metrics.iter().map(ProcedureMetricsEntry::snapshot).collect()
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

    pub(crate) fn record_request_received(
        &self,
        procedure: ProcedureLabels,
        command_queue_depth: usize,
    ) {
        self.requests_received.fetch_add(1, Ordering::Relaxed);
        self.procedure_metrics_entry(procedure).requests_received.fetch_add(1, Ordering::Relaxed);
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

    pub(crate) fn record_dispatch(
        &self,
        procedure: ProcedureLabels,
        queue_wait_micros: u64,
        dispatch_micros: u64,
    ) {
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
        let procedure_metrics = self.procedure_metrics_entry(procedure);
        procedure_metrics.requests_dispatched.fetch_add(1, Ordering::Relaxed);
        record_latency(
            &procedure_metrics.queue_wait_samples,
            &procedure_metrics.queue_wait_total_micros,
            &procedure_metrics.queue_wait_max_micros,
            queue_wait_micros,
        );
        record_latency(
            &procedure_metrics.dispatch_samples,
            &procedure_metrics.dispatch_total_micros,
            &procedure_metrics.dispatch_max_micros,
            dispatch_micros,
        );
    }

    pub(crate) fn record_reply_sent(
        &self,
        procedure: Option<ProcedureLabels>,
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
        if let Some(procedure) = procedure {
            let procedure_metrics = self.procedure_metrics_entry(procedure);
            procedure_metrics.replies_sent.fetch_add(1, Ordering::Relaxed);
            record_latency(
                &procedure_metrics.total_latency_samples,
                &procedure_metrics.total_latency_total_micros,
                &procedure_metrics.total_latency_max_micros,
                total_latency_micros,
            );
            record_latency(
                &procedure_metrics.dispatch_to_write_samples,
                &procedure_metrics.dispatch_to_write_total_micros,
                &procedure_metrics.dispatch_to_write_max_micros,
                dispatch_to_write_micros,
            );
        }
    }

    pub(crate) fn record_reply_failure(&self, procedure: Option<ProcedureLabels>) {
        self.reply_failures.fetch_add(1, Ordering::Relaxed);
        if let Some(procedure) = procedure {
            self.procedure_metrics_entry(procedure).reply_failures.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn procedure_metrics_entry(&self, procedure: ProcedureLabels) -> &ProcedureMetricsEntry {
        let index = PROCEDURE_LABELS
            .iter()
            .position(|labels| *labels == procedure)
            .expect("procedure labels must exist in metrics registry");
        &self.procedure_metrics[index]
    }
}

impl Default for ServerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerMetricsSnapshot {
    /// Encodes the snapshot in Prometheus text format.
    pub fn encode_prometheus(&self) -> String {
        encode_metrics(self, &[], false)
    }

    /// Encodes the snapshot in OpenMetrics text format.
    pub fn encode_openmetrics(&self) -> String {
        encode_metrics(self, &[], true)
    }
}

fn encode_metrics(
    snapshot: &ServerMetricsSnapshot,
    procedure_snapshots: &[ProcedureMetricsSnapshot],
    openmetrics: bool,
) -> String {
    let mut output = String::new();

    append_counter(
        &mut output,
        "nfs_mamont_requests_received_total",
        "Requests accepted into the command queue.",
        snapshot.requests_received,
    );
    append_counter(
        &mut output,
        "nfs_mamont_requests_rejected_total",
        "Requests rejected during parsing or reply serialization.",
        snapshot.requests_rejected,
    );
    append_counter(
        &mut output,
        "nfs_mamont_requests_dispatched_total",
        "Requests dispatched to protocol handlers.",
        snapshot.requests_dispatched,
    );
    append_counter(
        &mut output,
        "nfs_mamont_replies_sent_total",
        "Replies successfully written to the client socket.",
        snapshot.replies_sent,
    );
    append_counter(
        &mut output,
        "nfs_mamont_reply_failures_total",
        "Replies dropped before reaching the client socket.",
        snapshot.reply_failures,
    );

    append_gauge(
        &mut output,
        "nfs_mamont_command_queue_depth",
        "Current command queue depth.",
        snapshot.command_queue_depth.current,
    );
    append_gauge(
        &mut output,
        "nfs_mamont_command_queue_depth_peak",
        "Peak command queue depth.",
        snapshot.command_queue_depth.peak,
    );
    append_gauge(
        &mut output,
        "nfs_mamont_result_queue_depth",
        "Current result queue depth.",
        snapshot.result_queue_depth.current,
    );
    append_gauge(
        &mut output,
        "nfs_mamont_result_queue_depth_peak",
        "Peak result queue depth.",
        snapshot.result_queue_depth.peak,
    );
    append_gauge(
        &mut output,
        "nfs_mamont_in_flight_requests",
        "Current number of in-flight requests.",
        snapshot.in_flight_requests.current,
    );
    append_gauge(
        &mut output,
        "nfs_mamont_in_flight_requests_peak",
        "Peak number of in-flight requests.",
        snapshot.in_flight_requests.peak,
    );

    append_latency_metrics(&mut output, "queue_wait", &snapshot.queue_wait);
    append_latency_metrics(&mut output, "dispatch", &snapshot.dispatch);
    append_latency_metrics(&mut output, "total_latency", &snapshot.total_latency);
    append_latency_metrics(&mut output, "dispatch_to_write", &snapshot.dispatch_to_write);
    append_procedure_metrics(&mut output, procedure_snapshots);

    if openmetrics {
        output.push_str("# EOF\n");
    }

    output
}

fn append_counter(output: &mut String, name: &str, help: &str, value: u64) {
    let _ = writeln!(output, "# HELP {name} {help}");
    let _ = writeln!(output, "# TYPE {name} counter");
    let _ = writeln!(output, "{name} {value}");
}

fn append_gauge(output: &mut String, name: &str, help: &str, value: usize) {
    let _ = writeln!(output, "# HELP {name} {help}");
    let _ = writeln!(output, "# TYPE {name} gauge");
    let _ = writeln!(output, "{name} {value}");
}

fn append_latency_metrics(output: &mut String, prefix: &str, latency: &LatencySnapshot) {
    let samples_total_name = format!("nfs_mamont_{prefix}_samples_total");
    let micros_total_name = format!("nfs_mamont_{prefix}_micros_total");
    let average_name = format!("nfs_mamont_{prefix}_average_micros");
    let max_name = format!("nfs_mamont_{prefix}_max_micros");

    append_counter(
        output,
        &samples_total_name,
        "Number of recorded latency samples.",
        latency.samples,
    );
    append_counter(
        output,
        &micros_total_name,
        "Total observed latency in microseconds.",
        latency.total_micros,
    );
    append_gauge(
        output,
        &average_name,
        "Average observed latency in microseconds.",
        latency.average_micros as usize,
    );
    append_gauge(
        output,
        &max_name,
        "Maximum observed latency in microseconds.",
        latency.max_micros as usize,
    );
}

fn append_procedure_metrics(output: &mut String, procedure_snapshots: &[ProcedureMetricsSnapshot]) {
    append_labeled_counter_family(
        output,
        "nfs_mamont_procedure_requests_received_total",
        "Requests accepted into the command queue for each RPC procedure.",
        procedure_snapshots,
        |snapshot| snapshot.requests_received,
    );
    append_labeled_counter_family(
        output,
        "nfs_mamont_procedure_requests_dispatched_total",
        "Requests dispatched to protocol handlers for each RPC procedure.",
        procedure_snapshots,
        |snapshot| snapshot.requests_dispatched,
    );
    append_labeled_counter_family(
        output,
        "nfs_mamont_procedure_replies_sent_total",
        "Replies successfully written to the client socket for each RPC procedure.",
        procedure_snapshots,
        |snapshot| snapshot.replies_sent,
    );
    append_labeled_counter_family(
        output,
        "nfs_mamont_procedure_reply_failures_total",
        "Replies dropped before reaching the client socket for each RPC procedure.",
        procedure_snapshots,
        |snapshot| snapshot.reply_failures,
    );

    append_labeled_latency_family(
        output,
        "nfs_mamont_procedure_queue_wait_average_micros",
        "Average queue wait in microseconds for each RPC procedure.",
        procedure_snapshots,
        |snapshot| snapshot.queue_wait.average_micros,
    );
    append_labeled_latency_family(
        output,
        "nfs_mamont_procedure_dispatch_average_micros",
        "Average dispatch latency in microseconds for each RPC procedure.",
        procedure_snapshots,
        |snapshot| snapshot.dispatch.average_micros,
    );
    append_labeled_latency_family(
        output,
        "nfs_mamont_procedure_total_latency_average_micros",
        "Average total latency in microseconds for each RPC procedure.",
        procedure_snapshots,
        |snapshot| snapshot.total_latency.average_micros,
    );
    append_labeled_latency_family(
        output,
        "nfs_mamont_procedure_dispatch_to_write_average_micros",
        "Average write latency in microseconds for each RPC procedure.",
        procedure_snapshots,
        |snapshot| snapshot.dispatch_to_write.average_micros,
    );
    append_labeled_latency_family(
        output,
        "nfs_mamont_procedure_total_latency_max_micros",
        "Maximum total latency in microseconds for each RPC procedure.",
        procedure_snapshots,
        |snapshot| snapshot.total_latency.max_micros,
    );
}

fn append_labeled_counter_family(
    output: &mut String,
    name: &str,
    help: &str,
    snapshots: &[ProcedureMetricsSnapshot],
    value: impl Fn(&ProcedureMetricsSnapshot) -> u64,
) {
    let _ = writeln!(output, "# HELP {name} {help}");
    let _ = writeln!(output, "# TYPE {name} counter");
    for snapshot in snapshots {
        let _ =
            writeln!(output, "{name}{} {}", prometheus_labels(snapshot.labels), value(snapshot));
    }
}

fn append_labeled_latency_family(
    output: &mut String,
    name: &str,
    help: &str,
    snapshots: &[ProcedureMetricsSnapshot],
    value: impl Fn(&ProcedureMetricsSnapshot) -> u64,
) {
    let _ = writeln!(output, "# HELP {name} {help}");
    let _ = writeln!(output, "# TYPE {name} gauge");
    for snapshot in snapshots {
        let _ =
            writeln!(output, "{name}{} {}", prometheus_labels(snapshot.labels), value(snapshot));
    }
}

fn prometheus_labels(labels: ProcedureLabels) -> String {
    format!(
        "{{program=\"{}\",version=\"{}\",procedure=\"{}\"}}",
        escape_prometheus_label_value(labels.program),
        escape_prometheus_label_value(labels.version),
        escape_prometheus_label_value(labels.procedure),
    )
}

fn escape_prometheus_label_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
    reply_buffers: Arc<ReplyBufferPool>,
}

impl Default for ServerContext {
    fn default() -> Self {
        Self {
            settings: ServerSettings::default(),
            backend: None,
            exports: Arc::new(RwLock::new(ExportRegistry::default())),
            mounts: Arc::new(RwLock::new(MountRegistry::default())),
            metrics: Arc::new(ServerMetrics::default()),
            reply_buffers: Arc::new(ReplyBufferPool::default()),
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

    /// Returns the shared reply buffer pool.
    pub fn reply_buffers(&self) -> Arc<ReplyBufferPool> {
        Arc::clone(&self.reply_buffers)
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
    use std::sync::Arc;

    use super::{procedure_labels, ReplyBufferPool, RpcCallHeader, ServerMetrics, ServerSettings};

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
        let procedure = procedure_labels(RpcCallHeader {
            xid: 7,
            program: crate::nfsv3::NFS_PROGRAM,
            version: crate::nfsv3::NFS_VERSION,
            procedure: crate::nfsv3::READ,
            auth_flavor: super::AuthFlavor::None,
        });

        metrics.record_request_received(procedure, 3);
        metrics.record_request_rejected();
        metrics.observe_result_queue_depth(2);
        metrics.start_in_flight_request();
        metrics.record_dispatch(procedure, 10, 20);
        metrics.record_reply_sent(Some(procedure), 40, 15);
        metrics.record_reply_failure(Some(procedure));
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

    #[test]
    fn server_metrics_snapshot_encodes_openmetrics() {
        let metrics = ServerMetrics::new();
        let procedure = procedure_labels(RpcCallHeader {
            xid: 11,
            program: crate::mount::MOUNT_PROGRAM,
            version: crate::mount::MOUNT_VERSION,
            procedure: 1,
            auth_flavor: super::AuthFlavor::None,
        });

        metrics.record_request_received(procedure, 1);
        metrics.record_dispatch(procedure, 12, 34);
        metrics.record_reply_sent(Some(procedure), 56, 22);

        let text = metrics.encode_openmetrics();

        assert!(text.contains("# TYPE nfs_mamont_requests_received_total counter"));
        assert!(text.contains("nfs_mamont_requests_received_total 1"));
        assert!(text.contains("nfs_mamont_dispatch_micros_total 34"));
        assert!(text.contains("nfs_mamont_total_latency_average_micros 56"));
        assert!(text.contains(
            "nfs_mamont_procedure_requests_received_total{program=\"mount\",version=\"3\",procedure=\"mnt\"} 1"
        ));
        assert!(text.contains(
            "nfs_mamont_procedure_total_latency_average_micros{program=\"mount\",version=\"3\",procedure=\"mnt\"} 56"
        ));
        assert!(text.ends_with("# EOF\n"));
    }

    #[test]
    fn reply_buffer_pool_reuses_allocations() {
        let pool = Arc::new(ReplyBufferPool::new());
        let first_capacity = {
            let mut first = pool.acquire(128);
            first.as_mut_vec().extend_from_slice(&[1, 2, 3]);
            first.as_mut_vec().capacity()
        };

        let mut second = pool.acquire(64);
        assert!(second.as_slice().is_empty());
        assert!(second.as_mut_vec().capacity() >= first_capacity);
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
    pub(crate) procedure: ProcedureLabels,
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
        let procedure = procedure_labels(self.header);
        RpcCommand {
            context: RequestContext {
                connection,
                header: self.header,
                procedure,
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
    pub(crate) procedure: Option<ProcedureLabels>,
    pub span: Span,
    pub received_at: Instant,
    pub dispatched_at: Option<Instant>,
}

impl ReplyEnvelope {
    pub(crate) fn new(
        result: CommandResult,
        procedure: Option<ProcedureLabels>,
        span: Span,
        received_at: Instant,
        dispatched_at: Option<Instant>,
    ) -> Self {
        Self { result, procedure, span, received_at, dispatched_at }
    }
}

pub enum ReplyPayload {
    Buffer(OwnedReplyBuffer),
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
