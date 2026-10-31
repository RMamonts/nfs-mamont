//! Research crate for the `nfs-mamont` buffer allocator.
//!
//! Contains:
//! - [`pool`] --- a generic async pool (one mutex, FIFO waiters, no per-allocation heap
//!   traffic) parameterised by a free-space manager ([`pool::Core`]);
//! - [`fixed`] --- fixed-size blocks with a LIFO free list (same semantics as upstream
//!   `Impl`, cheaper bookkeeping);
//! - [`lockfree`] --- fixed-size blocks in a lock-free LIFO stack with O(1) batch
//!   pop/push and an atomic budget (mutex only while somebody waits);
//! - [`buddy`] --- 4 KiB pages managed by a buddy system that hands out *exactly sized,
//!   contiguous* runs (like Linux `alloc_pages_exact`), optionally with hot caches;
//! - [`harness`] / [`rss`] --- tiny measurement helpers used by the binaries in `src/bin`.

pub mod arena;
pub mod buddy;
pub mod fixed;
pub mod harness;
pub mod lockfree;
pub mod pool;
pub mod rss;
pub mod runs;

use std::num::NonZeroUsize;

pub use buddy::BuddyCore;
pub use fixed::{FifoCore, FixedCore};
pub use lockfree::{LfBuf, LockFreeAllocator};
pub use pool::{PoolAllocator, PoolBuf};

/// Pool with fixed-size blocks and LIFO reuse.
pub type FixedAllocator = PoolAllocator<FixedCore>;
/// Pool with 4 KiB pages and contiguous exact-size runs.
pub type BuddyAllocator = PoolAllocator<BuddyCore>;

pub const PAGE: usize = 4096;

/// Builds a [`FixedAllocator`] with `count` blocks of `size` bytes.
pub fn fixed(size: usize, count: usize, huge_pages: bool) -> FixedAllocator {
    let arena = arena::Arena::new(size * count, huge_pages);
    PoolAllocator::new(FixedCore::new(size, count), arena)
}

/// Builds a [`BuddyAllocator`] managing `bytes` bytes (rounded down to pages).
pub fn buddy(bytes: usize, huge_pages: bool) -> BuddyAllocator {
    let pages = bytes / PAGE;
    let arena = arena::Arena::new(pages * PAGE, huge_pages);
    PoolAllocator::new(BuddyCore::new(PAGE, pages), arena)
}

/// Same as [`buddy`] with a per-order hot cache of exact blocks.
pub fn buddy_cached(bytes: usize, huge_pages: bool) -> BuddyAllocator {
    let pages = bytes / PAGE;
    let arena = arena::Arena::new(pages * PAGE, huge_pages);
    PoolAllocator::new(BuddyCore::with_cache(PAGE, pages, buddy::CACHE_CAP), arena)
}

/// Lock-free LIFO pool of `count` fixed blocks of `size` bytes.
pub fn lockfree(size: usize, count: usize, huge_pages: bool) -> LockFreeAllocator {
    LockFreeAllocator::new(size, count, huge_pages)
}

/// Control variant: fixed blocks recycled FIFO (like upstream), page-aligned unless
/// `misalign` is non-zero.
pub fn fixed_fifo(
    size: usize,
    count: usize,
    misalign: usize,
    huge_pages: bool,
) -> PoolAllocator<FifoCore> {
    let arena = arena::Arena::with_offset(size * count, huge_pages, misalign);
    PoolAllocator::new(FifoCore::new(size, count), arena)
}

/// Control variant: fixed blocks recycled LIFO with a deliberately misaligned base.
pub fn fixed_misaligned(size: usize, count: usize, misalign: usize) -> FixedAllocator {
    let arena = arena::Arena::with_offset(size * count, false, misalign);
    PoolAllocator::new(FixedCore::new(size, count), arena)
}

/// Upstream allocator (the one under study).
pub fn upstream(size: usize, count: usize) -> nfs_mamont::Impl {
    nfs_mamont::Impl::new(NonZeroUsize::new(size).unwrap(), NonZeroUsize::new(count).unwrap())
}

pub const fn nz(v: usize) -> NonZeroUsize {
    match NonZeroUsize::new(v) {
        Some(v) => v,
        None => panic!("zero"),
    }
}
