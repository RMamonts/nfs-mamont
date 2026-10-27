//! Optional instrumentation. The disabled implementation performs no measurements.

#[cfg(feature = "metrics")]
mod enabled;
#[cfg(feature = "metrics")]
pub(crate) use enabled::*;

#[cfg(not(feature = "metrics"))]
mod disabled {
    use std::future::Future;

    pub(crate) fn start() {}

    pub(crate) fn instrument_read<F: Future>(future: F) -> F {
        future
    }
    pub(crate) fn instrument_write<F: Future>(future: F) -> F {
        future
    }
    pub(crate) fn instrument_vfs<F: Future>(future: F) -> F {
        future
    }
    pub(crate) struct ReadMetrics;
    impl ReadMetrics {
        pub(crate) fn new() -> Self {
            Self
        }
        pub(crate) fn record_cycle(&self) {}
    }

    #[derive(Clone, Copy)]
    pub struct Timestamp;

    impl Timestamp {
        pub(crate) fn now() -> Self {
            Self
        }
    }

    pub(crate) struct PoolMetrics;

    impl PoolMetrics {
        pub(crate) fn new<T: Send + 'static>(_: &async_channel::Receiver<T>, _: usize) -> Self {
            Self
        }
        pub(crate) fn worker(&self, _: usize) -> WorkerMetrics {
            WorkerMetrics
        }
    }

    pub(crate) struct WorkerMetrics;

    impl WorkerMetrics {
        pub(crate) fn begin(&self, _: Timestamp) -> RequestGuard {
            RequestGuard
        }
        pub(crate) fn record_request(&mut self, _: &'static str, _: Option<u8>, _: Timestamp) {}
    }

    pub(crate) struct RequestGuard;

    #[derive(Debug)]
    pub(crate) struct AllocatorMetrics;

    impl AllocatorMetrics {
        pub(crate) fn new(_: usize) -> Self {
            Self
        }
        pub(crate) async fn acquire<F: Future>(&self, future: F) -> F::Output {
            future.await
        }
        pub(crate) fn allocate(&self, _: usize) {}
        pub(crate) fn release(&self, _: usize) {}
    }
}

#[cfg(not(feature = "metrics"))]
pub(crate) use disabled::*;
