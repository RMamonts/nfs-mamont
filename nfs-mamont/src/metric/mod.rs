//! Server metrics recorded through the [`metrics`](https://docs.rs/metrics) facade.
//!
//! With the `metrics` feature disabled, [`NfsMetrics`] and its helpers are zero-sized
//! no-ops, so they should be compiled away.
#![allow(dead_code, unused_imports)] // TODO(artemkhel): remove once it's wired

mod allocator;

#[cfg(all(test, feature = "metrics"))]
mod tests;

pub use allocator::{AllocTimer, AllocatorMetrics};

pub use nfs_metrics::*;

/// Stage of an NFS request between the five timestamps of its pipeline:
/// parsed (T0) → picked by a VFS worker (T1) → backend done (T2) →
/// picked by the write task (T3) → reply written (T4).
#[derive(Clone, Copy)]
pub enum Stage {
    /// T0 → T1: waiting in the VFS pool queue.
    VfsQueue,
    /// T1 → T2: executing in the VFS worker, backend included.
    Vfs,
    /// T2 → T3: waiting in the connection reply queue.
    ReplyQueue,
    /// T3 → T4: serializing and writing the reply to the socket.
    Write,
}

impl Stage {
    #[cfg_attr(not(feature = "metrics"), allow(dead_code))]
    const ALL: [Stage; 4] = [Stage::VfsQueue, Stage::Vfs, Stage::ReplyQueue, Stage::Write];

    #[cfg_attr(not(feature = "metrics"), allow(dead_code))]
    fn as_str(self) -> &'static str {
        match self {
            Stage::VfsQueue => "vfs_queue",
            Stage::Vfs => "vfs",
            Stage::ReplyQueue => "reply_queue",
            Stage::Write => "write",
        }
    }
}

#[cfg(feature = "metrics")]
mod nfs_metrics {
    use std::num::NonZeroU64;
    use std::time::{Duration, Instant};

    use metrics::{
        counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram, Counter,
        Gauge, Histogram, Unit,
    };

    use super::Stage;

    pub(super) const STAGE_SECONDS: &str = "nfs_mamont_rpc_stage_seconds";
    pub(super) const DURATION_SECONDS: &str = "nfs_mamont_rpc_duration_seconds";
    pub(super) const REQUESTS_TOTAL: &str = "nfs_mamont_rpc_requests_total";
    pub(super) const VFS_IN_FLIGHT: &str = "nfs_mamont_vfs_in_flight";

    /// Pre-registered metric handles.
    ///
    /// Handles are bound to the recorder installed when [`NfsMetrics::new`] is called,
    /// so the recorder must be installed before that.
    pub struct NfsMetrics {
        /// Reference point for [`Timer`] timestamps.
        epoch: Instant,
        /// Indexed by [`Stage`].
        stage: [Histogram; 4],
        duration: Histogram,
        requests_ok: Counter,
        requests_err: Counter,
        vfs_in_flight: Gauge,
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
            describe_gauge!(
                VFS_IN_FLIGHT,
                Unit::Count,
                "NFS requests executing in the VFS worker pool"
            );

            Self {
                epoch: Instant::now(),
                stage: Stage::ALL.map(|stage| histogram!(STAGE_SECONDS, "stage" => stage.as_str())),
                duration: histogram!(DURATION_SECONDS),
                requests_ok: counter!(REQUESTS_TOTAL, "status" => "ok"),
                requests_err: counter!(REQUESTS_TOTAL, "status" => "error"),
                vfs_in_flight: gauge!(VFS_IN_FLIGHT),
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

        /// Counts a request executing in the VFS pool until the guard is dropped.
        #[inline]
        pub fn vfs_in_flight(&self) -> InFlightGuard<'_> {
            self.vfs_in_flight.increment(1.0);
            InFlightGuard(&self.vfs_in_flight)
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

    use super::Stage;

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
        pub fn vfs_in_flight(&self) -> InFlightGuard<'_> {
            InFlightGuard(PhantomData)
        }
    }
}
