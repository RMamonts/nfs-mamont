//! Generic async buffer pool.
//!
//! Design differences from upstream `nfs_mamont::Impl`:
//! - one `std::sync::Mutex` guards both the free-space manager and the FIFO wait
//!   queue: `allocate` = 1 lock, `drop` = 1 lock (upstream: semaphore CAS + n
//!   `ArrayQueue` CASes + `Vec` malloc on allocation; n CASes + the semaphore's
//!   wait-list mutex + `Vec` free on drop);
//! - the chunk list lives inline in the buffer ([`Runs`]), so the fast path does
//!   no heap allocation;
//! - waiters are only touched when they exist; permits are handed to the head of
//!   the queue directly (strict FIFO, like tokio's semaphore).

use std::collections::VecDeque;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use nfs_mamont::{Allocator, Buffer};

use crate::arena::Arena;
use crate::runs::{Run, Runs};

/// Free-space manager plugged into [`PoolAllocator`].
pub trait Core: Send + 'static {
    /// Allocation granule in bytes.
    fn unit(&self) -> usize;
    /// Number of units managed.
    fn total_units(&self) -> usize;
    /// Number of currently free units.
    fn free_units(&self) -> usize;
    /// Allocates exactly `units` units into `out`.
    ///
    /// Called only when `free_units() >= units`, and must succeed then.
    fn alloc(&mut self, units: usize, out: &mut Runs);
    /// Returns previously allocated runs.
    fn free(&mut self, runs: &[Run]);
}

pub(crate) struct Slot {
    pub(crate) waker: Option<Waker>,
    pub(crate) grant: Option<Runs>,
}

pub(crate) struct Waiter {
    pub(crate) units: usize,
    pub(crate) slot: Mutex<Slot>,
}

struct State<C> {
    core: C,
    waiters: VecDeque<Arc<Waiter>>,
}

pub struct Shared<C: Core> {
    state: Mutex<State<C>>,
    arena: Arena,
    unit: usize,
}

impl<C: Core> Shared<C> {
    /// Hands free units to queued waiters in FIFO order. Must be called with the
    /// state locked; returns the wakers to call after unlocking.
    fn serve(state: &mut State<C>) -> Vec<Waker> {
        let mut wakers = Vec::new();
        while let Some(head) = state.waiters.front() {
            if state.core.free_units() < head.units {
                break;
            }
            let head = state.waiters.pop_front().unwrap();
            let mut runs = Runs::new();
            state.core.alloc(head.units, &mut runs);
            let mut slot = head.slot.lock().unwrap();
            slot.grant = Some(runs);
            if let Some(w) = slot.waker.take() {
                wakers.push(w);
            }
        }
        wakers
    }

    fn release(&self, runs: &Runs) {
        let mut state = self.state.lock().unwrap();
        state.core.free(runs.as_slice());
        if state.waiters.is_empty() {
            return;
        }
        let wakers = Self::serve(&mut state);
        drop(state);
        for w in wakers {
            w.wake();
        }
    }
}

/// Future returned by [`PoolAllocator::allocate`] while it waits for memory.
struct Acquire<'a, C: Core> {
    shared: &'a Shared<C>,
    units: usize,
    waiter: Option<Arc<Waiter>>,
}

impl<C: Core> Future for Acquire<'_, C> {
    type Output = Runs;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Runs> {
        let this = &mut *self;
        if let Some(waiter) = &this.waiter {
            let mut slot = waiter.slot.lock().unwrap();
            if let Some(grant) = slot.grant.take() {
                drop(slot);
                this.waiter = None;
                return Poll::Ready(grant);
            }
            match &slot.waker {
                Some(w) if w.will_wake(cx.waker()) => {}
                _ => slot.waker = Some(cx.waker().clone()),
            }
            return Poll::Pending;
        }

        let mut state = this.shared.state.lock().unwrap();
        if state.waiters.is_empty() && state.core.free_units() >= this.units {
            let mut runs = Runs::new();
            state.core.alloc(this.units, &mut runs);
            return Poll::Ready(runs);
        }
        let waiter = Arc::new(Waiter {
            units: this.units,
            slot: Mutex::new(Slot { waker: Some(cx.waker().clone()), grant: None }),
        });
        state.waiters.push_back(Arc::clone(&waiter));
        drop(state);
        this.waiter = Some(waiter);
        Poll::Pending
    }
}

impl<C: Core> Drop for Acquire<'_, C> {
    /// Cancellation: leave the queue, give back a grant that was never collected and
    /// let the waiters behind us proceed.
    fn drop(&mut self) {
        let Some(waiter) = self.waiter.take() else { return };
        let mut state = self.shared.state.lock().unwrap();
        if let Some(pos) = state.waiters.iter().position(|w| Arc::ptr_eq(w, &waiter)) {
            state.waiters.remove(pos);
        }
        let grant = waiter.slot.lock().unwrap().grant.take();
        if let Some(grant) = grant {
            state.core.free(grant.as_slice());
        }
        let wakers = Shared::serve(&mut state);
        drop(state);
        for w in wakers {
            w.wake();
        }
    }
}

pub struct PoolAllocator<C: Core> {
    shared: Arc<Shared<C>>,
    capacity: NonZeroUsize,
}

impl<C: Core> PoolAllocator<C> {
    pub fn new(core: C, arena: Arena) -> Self {
        let unit = core.unit();
        let total = core.total_units() * unit;
        assert!(total <= arena.len(), "core manages more memory than the arena holds");
        let capacity = NonZeroUsize::new(total).expect("empty pool");
        let state = Mutex::new(State { core, waiters: VecDeque::new() });
        Self { shared: Arc::new(Shared { state, arena, unit }), capacity }
    }

    /// Free bytes right now (for tests and statistics).
    pub fn free_bytes(&self) -> usize {
        let state = self.shared.state.lock().unwrap();
        state.core.free_units() * self.shared.unit
    }

    pub fn waiters(&self) -> usize {
        self.shared.state.lock().unwrap().waiters.len()
    }

    pub fn base_addr(&self) -> usize {
        self.shared.arena.base() as usize
    }
}

impl<C: Core> Allocator for PoolAllocator<C> {
    type Buffer = PoolBuf<C>;

    async fn allocate(&self, size: NonZeroUsize) -> Option<PoolBuf<C>> {
        if size > self.capacity {
            return None;
        }
        let units = size.get().div_ceil(self.shared.unit);
        let runs = Acquire { shared: &self.shared, units, waiter: None }.await;
        Some(PoolBuf { shared: Some(Arc::clone(&self.shared)), runs, len: size.get() })
    }

    fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }
}

/// Buffer handed out by [`PoolAllocator`]; returns its memory on drop.
pub struct PoolBuf<C: Core> {
    shared: Option<Arc<Shared<C>>>,
    runs: Runs,
    len: usize,
}

impl<C: Core> PoolBuf<C> {
    /// Number of chunks (iovecs) this buffer consists of.
    pub fn chunk_count(&self) -> usize {
        self.chunks().count()
    }

    /// Bytes of pool memory held by this buffer (>= `len`).
    pub fn reserved(&self) -> usize {
        match &self.shared {
            Some(s) => self.runs.units() * s.unit,
            None => 0,
        }
    }

    #[inline]
    fn geometry(&self) -> (usize, usize) {
        match &self.shared {
            Some(s) => (s.arena.base() as usize, s.unit),
            None => (0, 0),
        }
    }
}

impl<C: Core> Drop for PoolBuf<C> {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.take() {
            shared.release(&self.runs);
        }
    }
}

impl<C: Core> Buffer for PoolBuf<C> {
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
        let (base, unit) = self.geometry();
        let mut left = self.len;
        self.runs.as_slice().iter().map_while(move |run| {
            if left == 0 {
                return None;
            }
            let n = (run.len as usize * unit).min(left);
            left -= n;
            // SAFETY: runs lie inside the arena and belong to this buffer only.
            Some(unsafe {
                std::slice::from_raw_parts((base + run.start as usize * unit) as *const u8, n)
            })
        })
    }

    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
        let (base, unit) = self.geometry();
        let mut left = self.len;
        self.runs.as_slice().iter().map_while(move |run| {
            if left == 0 {
                return None;
            }
            let n = (run.len as usize * unit).min(left);
            left -= n;
            // SAFETY: runs are disjoint, inside the arena, and `&mut self` is unique.
            Some(unsafe {
                std::slice::from_raw_parts_mut((base + run.start as usize * unit) as *mut u8, n)
            })
        })
    }

    fn len(&self) -> usize {
        self.len
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn empty() -> Self {
        PoolBuf { shared: None, runs: Runs::new(), len: 0 }
    }
}
