//! [`BudgetedAllocator`]: the allocator the parser of one connection allocates from.
//!
//! It charges every allocation to the data limit of the connection and to the global
//! data budget *before* calling the real allocator, so the read task parks on the
//! budgets, which are fair between connections and bounded per connection, instead of
//! on the pool semaphore of the allocator. The charge is handed to the read task with
//! [`BudgetedAllocator::take_charge`] and travels in the in-flight token of the request
//! until its reply is written. With a data budget below the pool size the real
//! allocator never has to wait; when it does anyway (a backend keeping buffers, a
//! budget larger than the pool) the wait is counted in [`stats::ALLOCATOR_WAITS`].

use std::future::{poll_fn, Future};
use std::num::NonZeroUsize;
use std::pin::pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Instant;

use tokio::sync::OwnedSemaphorePermit;
use tracing::warn;

use crate::allocator::Allocator;
use crate::limits::Limits;
use crate::stats;
use crate::task::connection::limit::Limiter;

/// What one allocation took from the budgets.
pub struct Charge {
    /// Bytes charged to the connection, rounded up to the allocator granularity.
    pub bytes: u64,
    /// Units of the global data budget, if one is set.
    pub permit: Option<OwnedSemaphorePermit>,
}

/// Allocator of one connection, see the module docs.
pub struct BudgetedAllocator<A: Allocator> {
    inner: Arc<A>,
    limits: Arc<Limits>,
    limiter: Arc<Limiter>,
    /// Bytes charged to the connection so far; only the read task changes it.
    issued: AtomicU64,
    /// Charge of the allocation made while parsing the current request.
    charge: Mutex<Option<Charge>>,
}

/// Set once the allocator had to wait, so the warning is logged once per process.
static ALLOCATOR_WAIT_LOGGED: AtomicBool = AtomicBool::new(false);

impl<A: Allocator + Send + Sync> BudgetedAllocator<A> {
    /// Wraps `inner` for the connection whose budget `limiter` belongs to.
    pub fn new(inner: Arc<A>, limits: Arc<Limits>, limiter: Arc<Limiter>) -> Self {
        Self { inner, limits, limiter, issued: AtomicU64::new(0), charge: Mutex::new(None) }
    }

    /// Takes the charge of the allocation made since the last call, if any.
    pub fn take_charge(&self) -> Option<Charge> {
        self.charge.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take()
    }

    /// Waits until the connection may hold `bytes` more. A single request larger than
    /// the limit is let through when nothing else is in flight, so it cannot wait
    /// forever.
    ///
    /// # Returns
    ///
    /// `false` when the write task is gone, so the request will never be answered.
    async fn acquire_connection_bytes(&self, bytes: u64) -> bool {
        let Some(limit) = self.limits.max_data_per_connection else {
            return true;
        };
        let issued = self.issued.load(Ordering::Relaxed);
        let fits = |released: u64| {
            let in_flight = issued - released;
            in_flight == 0 || in_flight + bytes <= limit
        };
        if !fits(self.limiter.bytes_released()) {
            stats::CONNECTION_DATA_WAITS.fetch_add(1, Ordering::Relaxed);
            loop {
                // Same protocol as the in-flight slots: announce the wait, look again,
                // then park; a release checks the flag after its increment.
                self.limiter.set_waiting(true);
                if fits(self.limiter.bytes_released()) || self.limiter.is_closed() {
                    self.limiter.set_waiting(false);
                    break;
                }
                self.limiter.parked().await;
                self.limiter.set_waiting(false);
            }
            if self.limiter.is_closed() {
                return false;
            }
        }
        self.issued.store(issued + bytes, Ordering::Relaxed);
        true
    }

    fn release_connection_bytes(&self, bytes: u64) {
        self.issued.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Takes `units` of the global data budget, if one is set.
    ///
    /// # Returns
    ///
    /// `Err(())` when the request can never be satisfied.
    async fn acquire_global(&self, units: usize) -> Result<Option<OwnedSemaphorePermit>, ()> {
        let Some(budget) = &self.limits.data else {
            return Ok(None);
        };
        if units > budget.units {
            return Err(());
        }
        let needed = u32::try_from(units).map_err(|_| ())?;
        let permit = match Arc::clone(&budget.semaphore).try_acquire_many_owned(needed) {
            Ok(permit) => permit,
            Err(_) => {
                stats::DATA_BUDGET_WAITS.fetch_add(1, Ordering::Relaxed);
                Arc::clone(&budget.semaphore).acquire_many_owned(needed).await.map_err(|_| ())?
            }
        };
        Ok(Some(permit))
    }
}

impl<A: Allocator + Send + Sync> Allocator for BudgetedAllocator<A> {
    type Buffer = A::Buffer;

    async fn allocate(&self, size: NonZeroUsize) -> Option<A::Buffer> {
        if size > self.inner.capacity() {
            return None;
        }
        let granularity = self.limits.granularity.get();
        let units = size.get().div_ceil(granularity);
        let bytes = u64::try_from(units.checked_mul(granularity)?).ok()?;

        if !self.acquire_connection_bytes(bytes).await {
            return None;
        }
        let permit = match self.acquire_global(units).await {
            Ok(permit) => permit,
            Err(()) => {
                self.release_connection_bytes(bytes);
                return None;
            }
        };

        // With the budgets in place the allocator should find the memory right away; a
        // wait here means the budgets do not keep the pool from running dry.
        let mut allocation = pin!(self.inner.allocate(size));
        let buffer = match poll_fn(|cx| Poll::Ready(allocation.as_mut().poll(cx))).await {
            Poll::Ready(buffer) => buffer,
            Poll::Pending => {
                stats::ALLOCATOR_WAITS.fetch_add(1, Ordering::Relaxed);
                if !ALLOCATOR_WAIT_LOGGED.swap(true, Ordering::Relaxed) {
                    warn!(
                        size = size.get(),
                        "allocator had to wait: the data budget does not keep the pool \
                         from running dry"
                    );
                }
                let started = Instant::now();
                let buffer = allocation.await;
                let waited = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
                stats::ALLOCATOR_WAIT_NS.fetch_add(waited, Ordering::Relaxed);
                buffer
            }
        };
        let Some(buffer) = buffer else {
            self.release_connection_bytes(bytes);
            return None;
        };
        *self.charge.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some(Charge { bytes, permit });

        Some(buffer)
    }

    fn capacity(&self) -> NonZeroUsize {
        self.inner.capacity()
    }

    fn granularity(&self) -> NonZeroUsize {
        self.inner.granularity()
    }
}
