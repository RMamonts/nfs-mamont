//! Metrics of the built-in [`crate::allocator::Impl`].

pub use allocator_metrics::*;

#[cfg(feature = "metrics")]
mod allocator_metrics {
    use std::fmt;
    use std::time::Instant;

    use metrics::{
        counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram, Counter,
        Gauge, Histogram, Unit,
    };

    pub(super) const WAIT_SECONDS: &str = "nfs_mamont_allocator_wait_seconds";
    pub(super) const REQUEST_BYTES: &str = "nfs_mamont_allocator_request_bytes";
    pub(super) const REJECTED_TOTAL: &str = "nfs_mamont_allocator_rejected_total";
    pub(super) const CHUNKS_IN_USE: &str = "nfs_mamont_allocator_chunks_in_use";
    pub(super) const CHUNKS_CAPACITY: &str = "nfs_mamont_allocator_chunks_capacity";

    /// Pre-registered allocator metric handles.
    ///
    /// Handles are bound to the recorder installed when [`AllocatorMetrics::new`] is called.
    pub struct AllocatorMetrics {
        wait: Histogram,
        request_bytes: Histogram,
        rejected: Counter,
        chunks_in_use: Gauge,
    }

    /// Start of an allocation, taken before waiting for free chunks.
    pub struct AllocTimer(Instant);

    impl AllocatorMetrics {
        pub fn new(chunks: usize) -> Self {
            describe_histogram!(
                WAIT_SECONDS,
                Unit::Seconds,
                "Time an allocation waits for free allocator chunks"
            );
            describe_histogram!(REQUEST_BYTES, Unit::Bytes, "Requested allocation size");
            describe_counter!(
                REJECTED_TOTAL,
                Unit::Count,
                "Allocations rejected for exceeding allocator capacity"
            );
            describe_gauge!(CHUNKS_IN_USE, Unit::Count, "Allocator chunks lent out");
            describe_gauge!(CHUNKS_CAPACITY, Unit::Count, "Total allocator chunks");

            gauge!(CHUNKS_CAPACITY).set(chunks as f64);

            Self {
                wait: histogram!(WAIT_SECONDS),
                request_bytes: histogram!(REQUEST_BYTES),
                rejected: counter!(REJECTED_TOTAL),
                chunks_in_use: gauge!(CHUNKS_IN_USE),
            }
        }

        #[inline]
        pub fn start(&self) -> AllocTimer {
            AllocTimer(Instant::now())
        }

        /// Records a served allocation of `size` bytes taking `chunks` chunks.
        #[inline]
        pub fn allocated(&self, timer: AllocTimer, size: usize, chunks: usize) {
            self.wait.record(timer.0.elapsed());
            self.request_bytes.record(size as f64);
            self.chunks_in_use.increment(chunks as f64);
        }

        #[inline]
        pub fn rejected(&self) {
            self.rejected.increment(1);
        }

        /// Records `chunks` chunks returned to the pool.
        #[inline]
        pub fn released(&self, chunks: usize) {
            self.chunks_in_use.decrement(chunks as f64);
        }
    }

    // `AllocatorState` derives `Debug`, `metrics` crate handles do not have it
    impl fmt::Debug for AllocatorMetrics {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("AllocatorMetrics").finish_non_exhaustive()
        }
    }
}

#[cfg(not(feature = "metrics"))]
mod allocator_metrics {
    /// No-op allocator metrics: the `metrics` feature is disabled.
    #[derive(Debug)]
    pub struct AllocatorMetrics;

    /// No-op allocation timer.
    pub struct AllocTimer;

    impl AllocatorMetrics {
        pub fn new(_chunks: usize) -> Self {
            Self
        }

        #[inline]
        pub fn start(&self) -> AllocTimer {
            AllocTimer
        }

        #[inline]
        pub fn allocated(&self, _timer: AllocTimer, _size: usize, _chunks: usize) {}

        #[inline]
        pub fn rejected(&self) {}

        #[inline]
        pub fn released(&self, _chunks: usize) {}
    }
}
