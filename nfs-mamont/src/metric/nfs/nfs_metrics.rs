use std::future::Future;
use std::num::NonZeroU64;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use metrics::{
    counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram, Counter,
    Gauge, Histogram, Key, Unit,
};
use tokio::task::AbortHandle;
use tokio_metrics::{FutureMonitor, TaskMetricsReporterBuilder, TaskMonitor};

use super::consts::{
    CPU_SECONDS, DURATION_SECONDS, PROC_NAMES, REPLY_BYTES, REQUESTS_TOTAL, STAGE_SECONDS,
    SYSCALL_SECONDS, TOKIO_SAMPLING_INTERVAL, VFS_IN_FLIGHT, WAIT_SECONDS,
};
use super::io::{IoStats, IoTimer};
use crate::metric::{Stage, TaskKind};
use crate::vfs::file::BackendId;

/// Reply metrics of one NFSv3 procedure.
struct ReplyMetrics {
    cpu: Histogram,
    syscall: Histogram,
    wait: Histogram,
    bytes: Histogram,
}

impl ReplyMetrics {
    fn new(proc: &'static str) -> Self {
        Self {
            cpu: histogram!(CPU_SECONDS, "message" => "reply", "proc" => proc),
            syscall: histogram!(SYSCALL_SECONDS, "message" => "reply", "proc" => proc),
            wait: histogram!(WAIT_SECONDS, "message" => "reply", "proc" => proc),
            bytes: histogram!(REPLY_BYTES, "message" => "reply", "proc" => proc),
        }
    }
}

/// Call metrics of one NFSv3 procedure.
struct CallMetrics {
    cpu: Histogram,
    syscall: Histogram,
}

impl CallMetrics {
    fn new(proc: &'static str) -> Self {
        Self {
            cpu: histogram!(CPU_SECONDS, "message" => "call", "proc" => proc),
            syscall: histogram!(SYSCALL_SECONDS, "message" => "call", "proc" => proc),
        }
    }
}

/// Pre-registered metric handles.
///
/// Handles are bound to the recorder installed when [`NfsMetrics::new`] is called,
/// so the recorder must be installed before that.
/// Per-backend in-flight gauges are the exception: they are registered on first use, in the recorder installed at that time.
pub struct NfsMetrics {
    /// Reference point for [`Timer`] timestamps.
    epoch: Instant,
    /// Indexed by [`Stage`].
    stage: [Histogram; 3],
    duration: Histogram,
    requests_ok: Counter,
    requests_err: Counter,
    /// Indexed by [`BackendId`], registered on first use.
    vfs_in_flight: [OnceLock<Gauge>; BackendId::MAX as usize + 1],
    /// Indexed by NFSv3 procedure number.
    reply: [ReplyMetrics; 22],
    /// Indexed by NFSv3 procedure number.
    call: [CallMetrics; 22],
    /// Indexed by [`TaskKind`].
    task_monitors: [TaskMonitor; 2],
    /// Tokio task reporters, aborted on drop.
    reporters: Vec<AbortHandle>,
}

/// Request timestamps carried along with the request through the pipeline,
/// or [`Timer::NONE`] for requests that are not measured.
///
/// Nanoseconds since [`NfsMetrics`] epoch: 16 bytes instead of 32 for two
/// [`Instant`]s, and the `None` state is free thanks to [`NonZeroU64`].
pub struct Timer(Option<Marks>);

struct Marks {
    /// Request start (T0).
    start: NonZeroU64,
    /// Last [`NfsMetrics::lap`] mark.
    last: u64,
}

impl Timer {
    /// Timer of a request that is not measured.
    pub const NONE: Self = Self(None);
}

/// Decrements the in-flight gauge on drop.
pub struct InFlightGuard<'a>(&'a Gauge);

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.0.decrement(1.0);
    }
}

impl NfsMetrics {
    /// Describes and registers all metrics in the currently installed recorder and spawns the tokio task metrics reporters.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn new() -> Self {
        describe_histogram!(
            STAGE_SECONDS,
            Unit::Seconds,
            "Time an NFS request spends in a pipeline stage"
        );
        describe_histogram!(
            DURATION_SECONDS,
            Unit::Seconds,
            "NFS request time from parsed request to written reply"
        );
        describe_counter!(REQUESTS_TOTAL, Unit::Count, "Completed NFS requests by status");
        describe_gauge!(VFS_IN_FLIGHT, Unit::Count, "NFS requests executing in a VFS backend");
        describe_histogram!(
            CPU_SECONDS,
            Unit::Seconds,
            "Time spent (de)serializing a message outside of socket syscalls"
        );
        describe_histogram!(
            SYSCALL_SECONDS,
            Unit::Seconds,
            "Time spent in socket syscalls for a message"
        );
        describe_histogram!(
            WAIT_SECONDS,
            Unit::Seconds,
            "Time a message waited for the socket to become ready"
        );
        describe_histogram!(REPLY_BYTES, Unit::Bytes, "Size of reply messages");

        let task_monitors = TaskKind::ALL.map(|_| TaskMonitor::new());
        let mut reporters = Vec::new();
        for (kind, monitor) in TaskKind::ALL.into_iter().zip(&task_monitors) {
            let reporter = TaskMetricsReporterBuilder::new(move |name| {
                let name = name.replacen("tokio_", "nfsmamont_tokio_task_", 1);
                Key::from_parts(name, &[("task", kind.as_str())])
            })
            .with_interval(TOKIO_SAMPLING_INTERVAL)
            .describe()
            .build_with_monitor(monitor.clone());
            reporters.push(tokio::spawn(reporter.run()).abort_handle());
        }

        Self {
            epoch: Instant::now(),
            stage: Stage::ALL.map(|stage| histogram!(STAGE_SECONDS, "stage" => stage.as_str())),
            duration: histogram!(DURATION_SECONDS),
            requests_ok: counter!(REQUESTS_TOTAL, "status" => "ok"),
            requests_err: counter!(REQUESTS_TOTAL, "status" => "error"),
            vfs_in_flight: std::array::from_fn(|_| OnceLock::new()),
            reply: PROC_NAMES.map(ReplyMetrics::new),
            call: PROC_NAMES.map(CallMetrics::new),
            task_monitors,
            reporters,
        }
    }

    /// Nanoseconds since the epoch, never zero.
    #[inline]
    fn now(&self) -> NonZeroU64 {
        let nanos = self.epoch.elapsed().as_nanos() as u64;
        NonZeroU64::new(nanos.saturating_add(1)).unwrap_or(NonZeroU64::MIN)
    }

    /// Starts timing a request (T0).
    #[inline]
    pub fn start(&self) -> Timer {
        let now = self.now();
        Timer(Some(Marks { start: now, last: now.get() }))
    }

    /// Records the time since the previous mark as `stage` and moves the mark to now.
    #[inline]
    pub fn lap(&self, timer: &mut Timer, stage: Stage) {
        let Some(marks) = &mut timer.0 else { return };
        let now = self.now().get();
        self.stage[stage as usize].record(Duration::from_nanos(now.saturating_sub(marks.last)));
        marks.last = now;
    }

    /// Records the whole request duration and counts the request.
    #[inline]
    pub fn finish(&self, timer: Timer, is_ok: bool) {
        let Some(marks) = timer.0 else { return };
        let now = self.now().get();
        self.duration.record(Duration::from_nanos(now.saturating_sub(marks.start.get())));
        if is_ok {
            self.requests_ok.increment(1);
        } else {
            self.requests_err.increment(1);
        }
    }

    /// Counts a request executing in `backend` until the guard is dropped.
    #[inline]
    pub fn vfs_in_flight(&self, backend: BackendId) -> InFlightGuard<'_> {
        let gauge = self.vfs_in_flight[backend as usize]
            .get_or_init(|| gauge!(VFS_IN_FLIGHT, "backend" => backend.to_string()));
        gauge.increment(1.0);
        InFlightGuard(gauge)
    }

    /// Instruments `fut` with the task monitor of `kind`.
    #[inline]
    pub fn instrument<F: Future>(&self, kind: TaskKind, fut: F) -> impl Future<Output = F::Output> {
        self.task_monitors[kind as usize].instrument(fut)
    }

    /// Wraps a connection socket, so that its I/O time can be measured
    #[inline]
    pub fn io_timer<S>(&self, socket: S) -> (IoTimer<S>, Arc<IoStats>) {
        IoTimer::new(socket)
    }

    /// Awaits `reply`, which writes one reply through the [`IoTimer`] behind `io`, and
    /// records its CPU, syscall and socket wait times and reply size under `procedure`.
    /// Nothing is recorded for `None` or a failed write.
    pub async fn write_reply<T, F: Future<Output = std::io::Result<T>>>(
        &self,
        procedure: Option<u32>,
        io: &IoStats,
        reply: F,
    ) -> F::Output {
        let Some(procedure) = procedure else {
            let output = reply.await;
            // Reset the I/O of the unmeasured reply
            io.take();
            return output;
        };

        let (output, future_metrics) = FutureMonitor::new().instrument(reply).await;
        let (syscall, bytes) = io.take();
        if output.is_ok() {
            let metrics = &self.reply[procedure as usize];
            // IoTimer measures time inside socket's write polls ~= syscall time
            // Total future poll duration - syscall time = serialization time
            metrics.cpu.record(future_metrics.total_poll_duration.saturating_sub(syscall));
            metrics.syscall.record(syscall);
            metrics.wait.record(future_metrics.total_idle_duration);
            metrics.bytes.record(bytes as f64);
        }
        output
    }

    /// Awaits `call`, which reads one call through the [`IoTimer`] behind `io`, and
    /// records its CPU and syscall time under the procedure `procedure` returns for it.
    /// Nothing but bytes is recorded for `None`.
    ///
    /// One read can buffer several consecutive calls, so the first of them carries
    /// the syscall time and bytes of the rest.
    pub async fn read_call<F: Future>(
        &self,
        io: &IoStats,
        call: F,
        procedure: impl FnOnce(&F::Output) -> Option<u32>,
    ) -> F::Output {
        let (output, future_metrics) = FutureMonitor::new().instrument(call).await;
        let (syscall, _) = io.take();
        if let Some(procedure) = procedure(&output) {
            let metrics = &self.call[procedure as usize];
            // Syscalls run inside polls; the rest of the poll time is deserialization
            metrics.cpu.record(future_metrics.total_poll_duration.saturating_sub(syscall));
            metrics.syscall.record(syscall);
        }
        output
    }
}

impl Drop for NfsMetrics {
    fn drop(&mut self) {
        self.reporters.iter().for_each(AbortHandle::abort);
    }
}

impl Default for NfsMetrics {
    fn default() -> Self {
        Self::new()
    }
}
