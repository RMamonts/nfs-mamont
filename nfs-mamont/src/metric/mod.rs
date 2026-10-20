//! Server metrics recorded through the [`metrics`](https://docs.rs/metrics) facade.
//!
//! With the `metrics` feature disabled, [`NfsMetrics`] and its helpers are zero-sized
//! no-ops, so they should be compiled away.

mod allocator;

#[cfg(all(test, feature = "metrics"))]
mod tests;

pub use allocator::AllocatorMetrics;

pub use nfs_metrics::*;

/// Stage of an NFS request between the five timestamps of its pipeline:
/// received (T0) → parsed (T1) → picked by a VFS worker (T2) → backend done (T3) →
/// picked by the write task (T4) → reply written (T5).
#[derive(Clone, Copy)]
pub enum Stage {
    /// T0 → T1: RPC parsing
    Read,
    /// T1 → T2: waiting in the VFS pool queue.
    VfsQueue,
    /// T2 → T3: executing in the VFS worker, backend included.
    Vfs,
    /// T3 → T4: waiting in the connection reply queue.
    ReplyQueue,
    /// T4 → T5: serializing and writing the reply to the socket.
    Write,
}

impl Stage {
    #[cfg_attr(not(feature = "metrics"), allow(dead_code))]
    const ALL: [Stage; 5] =
        [Stage::Read, Stage::VfsQueue, Stage::Vfs, Stage::ReplyQueue, Stage::Write];

    #[cfg_attr(not(feature = "metrics"), allow(dead_code))]
    fn as_str(self) -> &'static str {
        match self {
            Stage::Read => "read",
            Stage::VfsQueue => "vfs_queue",
            Stage::Vfs => "vfs",
            Stage::ReplyQueue => "reply_queue",
            Stage::Write => "write",
        }
    }
}

/// Pipeline queue whose depth is tracked.
#[derive(Clone, Copy)]
pub enum Queue {
    /// Read tasks → VFS worker pool.
    Vfs,
    /// VFS workers → connection write tasks, summed over connections.
    Reply,
}

impl Queue {
    #[cfg_attr(not(feature = "metrics"), allow(dead_code))]
    const ALL: [Queue; 2] = [Queue::Vfs, Queue::Reply];

    #[cfg_attr(not(feature = "metrics"), allow(dead_code))]
    fn as_str(self) -> &'static str {
        match self {
            Queue::Vfs => "vfs",
            Queue::Reply => "reply",
        }
    }
}

#[cfg(feature = "metrics")]
mod nfs_metrics {
    use std::num::NonZeroU64;
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    use metrics::{
        counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram, Counter,
        Gauge, Histogram, Unit,
    };

    use super::{Queue, Stage};
    use crate::vfs::file::BackendId;

    pub(super) const STAGE_SECONDS: &str = "nfs_mamont_rpc_stage_seconds";
    pub(super) const DURATION_SECONDS: &str = "nfs_mamont_rpc_duration_seconds";
    pub(super) const REQUESTS_TOTAL: &str = "nfs_mamont_rpc_requests_total";
    pub(super) const VFS_IN_FLIGHT: &str = "nfs_mamont_vfs_in_flight";
    pub(super) const QUEUE_DEPTH: &str = "nfs_mamont_queue_depth";

    /// Pre-registered metric handles.
    ///
    /// Handles are bound to the recorder installed when [`NfsMetrics::new`] is called,
    /// so the recorder must be installed before that.
    /// Per-backend in-flight gauges are the exception: they are registered on first use, in the recorder installed at that time.
    pub struct NfsMetrics {
        /// Reference point for [`Timer`] timestamps.
        epoch: Instant,
        /// Indexed by [`Stage`].
        stage: [Histogram; 5],
        duration: Histogram,
        requests_ok: Counter,
        requests_err: Counter,
        /// Indexed by [`BackendId`], registered on first use.
        vfs_in_flight: [OnceLock<Gauge>; BackendId::MAX as usize + 1],
        /// Indexed by [`Queue`].
        queue_depth: [Gauge; 2],
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

        /// Whether the request is measured.
        #[inline]
        pub fn is_measured(&self) -> bool {
            self.0.is_some()
        }
    }

    /// Decrements the in-flight gauge on drop.
    pub struct InFlightGuard<'a>(&'a Gauge);

    impl Drop for InFlightGuard<'_> {
        fn drop(&mut self) {
            self.0.decrement(1.0);
        }
    }

    impl NfsMetrics {
        /// Describes and registers all metrics in the currently installed recorder.
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
            describe_gauge!(QUEUE_DEPTH, Unit::Count, "NFS requests waiting in a pipeline queue");

            Self {
                epoch: Instant::now(),
                stage: Stage::ALL.map(|stage| histogram!(STAGE_SECONDS, "stage" => stage.as_str())),
                duration: histogram!(DURATION_SECONDS),
                requests_ok: counter!(REQUESTS_TOTAL, "status" => "ok"),
                requests_err: counter!(REQUESTS_TOTAL, "status" => "error"),
                vfs_in_flight: std::array::from_fn(|_| OnceLock::new()),
                queue_depth: Queue::ALL.map(|queue| gauge!(QUEUE_DEPTH, "queue" => queue.as_str())),
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

        /// Counts a request as waiting in `queue`; pair with [`NfsMetrics::dequeued`].
        #[inline]
        pub fn enqueued(&self, queue: Queue) {
            self.queue_depth[queue as usize].increment(1.0);
        }

        /// Counts a request as no longer waiting in `queue`.
        #[inline]
        pub fn dequeued(&self, queue: Queue) {
            self.queue_depth[queue as usize].decrement(1.0);
        }
    }

    impl Default for NfsMetrics {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(not(feature = "metrics"))]
mod nfs_metrics {
    use std::marker::PhantomData;

    use super::{Queue, Stage};
    use crate::vfs::file::BackendId;

    /// No-op metrics: the `metrics` feature is disabled.
    #[derive(Default)]
    pub struct NfsMetrics;

    /// No-op timer.
    pub struct Timer;

    impl Timer {
        pub const NONE: Self = Self;

        #[inline]
        pub const fn is_measured(&self) -> bool {
            false
        }
    }

    /// No-op in-flight guard.
    pub struct InFlightGuard<'a>(PhantomData<&'a ()>);

    impl NfsMetrics {
        pub fn new() -> Self {
            Self
        }

        #[inline]
        pub fn start(&self) -> Timer {
            Timer
        }

        #[inline]
        pub fn lap(&self, _timer: &mut Timer, _stage: Stage) {}

        #[inline]
        pub fn finish(&self, _timer: Timer, _is_ok: bool) {}

        #[inline]
        pub fn vfs_in_flight(&self, _backend: BackendId) -> InFlightGuard<'_> {
            InFlightGuard(PhantomData)
        }

        #[inline]
        pub fn enqueued(&self, _queue: Queue) {}

        #[inline]
        pub fn dequeued(&self, _queue: Queue) {}
    }
}
