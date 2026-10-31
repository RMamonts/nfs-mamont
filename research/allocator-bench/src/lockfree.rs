//! Lock-free fixed-size pool: upstream semantics (one chunk per block), but
//! - free blocks form a LIFO Treiber stack of indices with a tagged head, so a
//!   request of `n` blocks is popped / pushed with ONE CAS (upstream: `n` CASes on
//!   `ArrayQueue` plus the semaphore);
//! - the budget is a plain atomic; the wait-queue mutex is touched only while
//!   somebody actually waits (upstream: tokio's semaphore takes its wait-list
//!   mutex on every `add_permits`, i.e. on every buffer drop);
//! - no per-allocation heap traffic (inline [`Runs`]).

use std::collections::VecDeque;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering::*};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use nfs_mamont::{Allocator, Buffer};

use crate::arena::Arena;
use crate::pool::{Slot, Waiter};
use crate::runs::{Run, Runs};

const NIL: u32 = u32::MAX;

pub struct Shared {
    arena: Arena,
    unit: usize,
    total: usize,
    /// `(tag << 32) | top_index`; the tag defeats ABA.
    head: AtomicU64,
    links: Box<[AtomicU32]>,
    /// Blocks that can be claimed right now. Invariant: blocks in the stack >= avail.
    avail: AtomicUsize,
    nwaiters: AtomicUsize,
    waiters: Mutex<VecDeque<Arc<Waiter>>>,
}

impl Shared {
    #[inline]
    fn try_claim(&self, n: usize) -> bool {
        let mut a = self.avail.load(SeqCst);
        loop {
            if a < n {
                return false;
            }
            match self.avail.compare_exchange_weak(a, a - n, SeqCst, SeqCst) {
                Ok(_) => return true,
                Err(x) => a = x,
            }
        }
    }

    /// Pops `n` blocks (already claimed through `avail`) with a single CAS.
    fn pop_n(&self, n: usize, out: &mut Runs) {
        let mut h = self.head.load(Acquire);
        loop {
            let first = h as u32;
            let mut last = first;
            let mut ok = first != NIL;
            let mut i = 1;
            while ok && i < n {
                last = self.links[last as usize].load(Relaxed);
                ok = last != NIL;
                i += 1;
            }
            if !ok {
                // Inconsistent snapshot (concurrent pop/push); retry with a fresh head.
                std::hint::spin_loop();
                h = self.head.load(Acquire);
                continue;
            }
            let next = self.links[last as usize].load(Relaxed);
            let new = (((h >> 32) + 1) << 32) | next as u64;
            match self.head.compare_exchange_weak(h, new, Acquire, Acquire) {
                Ok(_) => {
                    // The chain first..=last is ours now; nobody else writes its links.
                    let mut idx = first;
                    for _ in 0..n {
                        out.push(Run { start: idx, len: 1 });
                        idx = self.links[idx as usize].load(Relaxed);
                    }
                    return;
                }
                Err(x) => h = x,
            }
        }
    }

    /// Pushes the blocks of `runs` with a single CAS (first run ends up on top).
    fn push_all(&self, runs: &[Run]) {
        let n = runs.len();
        for i in 0..n - 1 {
            self.links[runs[i].start as usize].store(runs[i + 1].start, Relaxed);
        }
        let (first, last) = (runs[0].start, runs[n - 1].start);
        let mut h = self.head.load(Relaxed);
        loop {
            self.links[last as usize].store(h as u32, Relaxed);
            let new = (((h >> 32) + 1) << 32) | first as u64;
            match self.head.compare_exchange_weak(h, new, Release, Relaxed) {
                Ok(_) => return,
                Err(x) => h = x,
            }
        }
    }

    /// Serves queued waiters in FIFO order; the wait-queue lock must be held.
    fn serve(&self, q: &mut VecDeque<Arc<Waiter>>) -> Vec<Waker> {
        let mut wakers = Vec::new();
        while let Some(head) = q.front() {
            if !self.try_claim(head.units) {
                break;
            }
            let head = q.pop_front().unwrap();
            self.nwaiters.fetch_sub(1, SeqCst);
            let mut runs = Runs::new();
            self.pop_n(head.units, &mut runs);
            let mut slot = head.slot.lock().unwrap();
            slot.grant = Some(runs);
            if let Some(w) = slot.waker.take() {
                wakers.push(w);
            }
        }
        wakers
    }

    fn release(&self, runs: &Runs) {
        let s = runs.as_slice();
        if s.is_empty() {
            return;
        }
        self.push_all(s);
        self.avail.fetch_add(s.len(), SeqCst);
        if self.nwaiters.load(SeqCst) > 0 {
            let mut q = self.waiters.lock().unwrap();
            let wakers = self.serve(&mut q);
            drop(q);
            for w in wakers {
                w.wake();
            }
        }
    }
}

struct Acquire<'a> {
    shared: &'a Shared,
    units: usize,
    waiter: Option<Arc<Waiter>>,
}

impl Future for Acquire<'_> {
    type Output = Runs;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Runs> {
        let this = &mut *self;
        let shared = this.shared;
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
        if shared.nwaiters.load(SeqCst) == 0 && shared.try_claim(this.units) {
            let mut runs = Runs::new();
            shared.pop_n(this.units, &mut runs);
            return Poll::Ready(runs);
        }
        let mut q = shared.waiters.lock().unwrap();
        shared.nwaiters.fetch_add(1, SeqCst);
        if q.is_empty() && shared.try_claim(this.units) {
            shared.nwaiters.fetch_sub(1, SeqCst);
            drop(q);
            let mut runs = Runs::new();
            shared.pop_n(this.units, &mut runs);
            return Poll::Ready(runs);
        }
        let waiter = Arc::new(Waiter {
            units: this.units,
            slot: Mutex::new(Slot { waker: Some(cx.waker().clone()), grant: None }),
        });
        q.push_back(Arc::clone(&waiter));
        drop(q);
        this.waiter = Some(waiter);
        Poll::Pending
    }
}

impl Drop for Acquire<'_> {
    fn drop(&mut self) {
        let Some(waiter) = self.waiter.take() else { return };
        let shared = self.shared;
        let mut q = shared.waiters.lock().unwrap();
        if let Some(pos) = q.iter().position(|w| Arc::ptr_eq(w, &waiter)) {
            q.remove(pos);
            shared.nwaiters.fetch_sub(1, SeqCst);
        }
        let grant = waiter.slot.lock().unwrap().grant.take();
        if let Some(grant) = grant {
            shared.push_all(grant.as_slice());
            shared.avail.fetch_add(grant.len(), SeqCst);
        }
        let wakers = shared.serve(&mut q);
        drop(q);
        for w in wakers {
            w.wake();
        }
    }
}

pub struct LockFreeAllocator {
    shared: Arc<Shared>,
    capacity: NonZeroUsize,
}

impl LockFreeAllocator {
    pub fn new(unit: usize, count: usize, huge_pages: bool) -> Self {
        assert!(count > 0 && count < NIL as usize);
        let arena = Arena::new(unit * count, huge_pages);
        // Stack 0 -> 1 -> 2 -> ... so the first allocations are contiguous.
        let links: Box<[AtomicU32]> = (0..count as u32)
            .map(|i| AtomicU32::new(if i + 1 < count as u32 { i + 1 } else { NIL }))
            .collect();
        let shared = Shared {
            arena,
            unit,
            total: count,
            head: AtomicU64::new(0),
            links,
            avail: AtomicUsize::new(count),
            nwaiters: AtomicUsize::new(0),
            waiters: Mutex::new(VecDeque::new()),
        };
        Self { shared: Arc::new(shared), capacity: NonZeroUsize::new(unit * count).unwrap() }
    }

    pub fn free_bytes(&self) -> usize {
        self.shared.avail.load(SeqCst) * self.shared.unit
    }

    pub fn waiters(&self) -> usize {
        self.shared.waiters.lock().unwrap().len()
    }

    pub fn total_blocks(&self) -> usize {
        self.shared.total
    }
}

impl Allocator for LockFreeAllocator {
    type Buffer = LfBuf;

    async fn allocate(&self, size: NonZeroUsize) -> Option<LfBuf> {
        if size > self.capacity {
            return None;
        }
        let units = size.get().div_ceil(self.shared.unit);
        let runs = Acquire { shared: &self.shared, units, waiter: None }.await;
        Some(LfBuf { shared: Some(Arc::clone(&self.shared)), runs, len: size.get() })
    }

    fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }
}

pub struct LfBuf {
    shared: Option<Arc<Shared>>,
    runs: Runs,
    len: usize,
}

impl LfBuf {
    pub fn chunk_count(&self) -> usize {
        self.chunks().count()
    }

    #[inline]
    fn geometry(&self) -> (usize, usize) {
        match &self.shared {
            Some(s) => (s.arena.base() as usize, s.unit),
            None => (0, 0),
        }
    }
}

impl Drop for LfBuf {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.take() {
            shared.release(&self.runs);
        }
    }
}

impl Buffer for LfBuf {
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
        let (base, unit) = self.geometry();
        let mut left = self.len;
        self.runs.as_slice().iter().map_while(move |run| {
            if left == 0 {
                return None;
            }
            let n = (run.len as usize * unit).min(left);
            left -= n;
            // SAFETY: blocks lie inside the arena and belong to this buffer only.
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
            // SAFETY: blocks are disjoint, inside the arena, `&mut self` is unique.
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
        LfBuf { shared: None, runs: Runs::new(), len: 0 }
    }
}
