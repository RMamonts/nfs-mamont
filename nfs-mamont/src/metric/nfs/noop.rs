use std::future::Future;
use std::marker::PhantomData;

use crate::metric::{Stage, TaskKind};
use crate::vfs::file::BackendId;

/// No-op metrics: the `metrics` feature is disabled.
#[derive(Default)]
pub struct NfsMetrics;

/// No-op timer.
pub struct Timer;

impl Timer {
    pub const NONE: Self = Self;
}

/// No-op in-flight guard.
pub struct InFlightGuard<'a>(PhantomData<&'a ()>);

/// No-op I/O stats.
pub struct IoStats;

impl NfsMetrics {
    pub fn new() -> Self {
        Self
    }

    #[inline]
    pub fn lap(&self, _timer: &mut Timer, _stage: Stage) {}

    #[inline]
    pub fn finish(&self, _timer: Timer) {}

    #[inline]
    pub fn vfs_in_flight(&self, _backend: BackendId) -> InFlightGuard<'_> {
        InFlightGuard(PhantomData)
    }

    #[inline]
    pub fn instrument<F: Future>(&self, _kind: TaskKind, fut: F) -> F {
        fut
    }

    #[inline]
    pub fn io_timer<S>(&self, socket: S) -> (S, IoStats) {
        (socket, IoStats)
    }

    #[inline]
    pub fn write_reply<T, F: Future<Output = std::io::Result<T>>>(
        &self,
        _timer: &Timer,
        _procedure: Option<u32>,
        _io: &IoStats,
        reply: F,
    ) -> F {
        reply
    }

    #[inline]
    pub async fn read_call<F: Future>(
        &self,
        _io: &IoStats,
        call: F,
        _procedure: impl FnOnce(&F::Output) -> Option<u32>,
    ) -> (F::Output, Timer) {
        (call.await, Timer)
    }
}
