//! Buddy page allocator that returns *exactly sized contiguous* runs.
//!
//! A request of `n` pages takes an aligned block of `2^ceil(log2 n)` pages and
//! immediately gives the tail back (the `alloc_pages_exact` trick), so a buffer is a
//! single chunk with at most one page of rounding waste. Only when the pool is too
//! fragmented to provide such a block does the request fall back to several smaller
//! blocks (several chunks) --- the `Buffer` trait is chunked anyway.
//!
//! All operations are O(log N); free lists are LIFO, so freed memory is reused hot.

use crate::pool::Core;
use crate::runs::{Run, Runs};

const NONE: u32 = u32::MAX;
const NOT_FREE: u8 = u8::MAX;
const MAX_ORDERS: usize = 32;

pub struct BuddyCore {
    page: usize,
    total: usize,
    max_order: u32,
    /// Head page of the free list of every order.
    heads: [u32; MAX_ORDERS],
    /// Bit `o` is set iff `heads[o] != NONE`.
    nonempty: u32,
    next: Vec<u32>,
    prev: Vec<u32>,
    /// Order of the free block starting at this page, `NOT_FREE` otherwise.
    order: Vec<u8>,
    free_pages: usize,
    /// Optional per-order LIFO caches of exact power-of-two blocks that are *not*
    /// merged on free: steady-state alloc/free of popular sizes becomes O(1) and the
    /// critical section tiny. Cached pages still count as free; caches are flushed
    /// (merged) when a request cannot be served otherwise.
    cache: Vec<Vec<u32>>,
    cache_cap: usize,
}

/// Blocks kept per order in the hot cache.
pub const CACHE_CAP: usize = 32;

#[inline]
fn floor_log2(x: usize) -> u32 {
    usize::BITS - 1 - x.leading_zeros()
}

#[inline]
fn ceil_log2(x: usize) -> u32 {
    if x <= 1 {
        0
    } else {
        floor_log2(x - 1) + 1
    }
}

impl BuddyCore {
    pub fn new(page: usize, total_pages: usize) -> Self {
        Self::with_cache(page, total_pages, 0)
    }

    /// Like [`Self::new`] with a hot cache of `cache_cap` blocks per order.
    pub fn with_cache(page: usize, total_pages: usize, cache_cap: usize) -> Self {
        assert!(page.is_power_of_two());
        assert!(total_pages > 0 && total_pages < (1 << 31));
        let mut core = Self {
            page,
            total: total_pages,
            max_order: floor_log2(total_pages),
            heads: [NONE; MAX_ORDERS],
            nonempty: 0,
            next: vec![NONE; total_pages],
            prev: vec![NONE; total_pages],
            order: vec![NOT_FREE; total_pages],
            free_pages: total_pages,
            cache: (0..MAX_ORDERS).map(|_| Vec::new()).collect(),
            cache_cap,
        };
        core.free_range(0, total_pages);
        core
    }

    #[inline]
    fn push(&mut self, b: u32, o: u32) {
        let h = self.heads[o as usize];
        self.next[b as usize] = h;
        self.prev[b as usize] = NONE;
        if h != NONE {
            self.prev[h as usize] = b;
        }
        self.heads[o as usize] = b;
        self.order[b as usize] = o as u8;
        self.nonempty |= 1 << o;
    }

    #[inline]
    fn unlink(&mut self, b: u32, o: u32) {
        let (n, p) = (self.next[b as usize], self.prev[b as usize]);
        if p != NONE {
            self.next[p as usize] = n;
        } else {
            self.heads[o as usize] = n;
            if n == NONE {
                self.nonempty &= !(1 << o);
            }
        }
        if n != NONE {
            self.prev[n as usize] = p;
        }
        self.order[b as usize] = NOT_FREE;
    }

    /// Takes an aligned block of `2^o` pages, splitting a larger one if needed.
    #[inline]
    fn alloc_block(&mut self, o: u32) -> Option<u32> {
        let avail = self.nonempty >> o;
        if avail == 0 {
            return None;
        }
        let mut k = o + avail.trailing_zeros();
        let b = self.heads[k as usize];
        self.unlink(b, k);
        while k > o {
            k -= 1;
            self.push(b + (1 << k), k);
        }
        Some(b)
    }

    /// Returns an aligned block of `2^o` pages, merging it with free buddies.
    #[inline]
    fn free_block(&mut self, mut b: u32, mut o: u32) {
        while o < self.max_order {
            let buddy = b ^ (1 << o);
            if buddy as usize >= self.total || self.order[buddy as usize] != o as u8 {
                break;
            }
            self.unlink(buddy, o);
            b &= !(1 << o);
            o += 1;
        }
        self.push(b, o);
    }

    /// Frees an arbitrary page range by splitting it into maximal aligned blocks.
    fn free_range(&mut self, start: usize, end: usize) {
        let mut p = start;
        while p < end {
            let mut o =
                if p == 0 { self.max_order } else { p.trailing_zeros().min(self.max_order) };
            while p + (1 << o) > end {
                o -= 1;
            }
            self.free_block(p as u32, o);
            p += 1 << o;
        }
    }

    /// Merges all cached blocks back into the buddy free lists.
    pub fn flush(&mut self) {
        for o in 0..MAX_ORDERS {
            while let Some(b) = self.cache[o].pop() {
                self.free_block(b, o as u32);
            }
        }
    }

    fn cached_pages(&self) -> usize {
        self.cache.iter().enumerate().map(|(o, c)| c.len() << o).sum()
    }

    /// Number of free blocks per order (for tests / fragmentation statistics).
    pub fn histogram(&self) -> Vec<usize> {
        (0..=self.max_order)
            .map(|o| {
                let mut n = 0;
                let mut b = self.heads[o as usize];
                while b != NONE {
                    n += 1;
                    b = self.next[b as usize];
                }
                n
            })
            .collect()
    }

    /// Checks internal consistency (tests only; O(N)).
    pub fn check(&self) {
        let mut covered = vec![false; self.total];
        let mut free = 0;
        for o in 0..=self.max_order {
            let mut b = self.heads[o as usize];
            assert_eq!(b != NONE, self.nonempty & (1 << o) != 0);
            let mut prev = NONE;
            while b != NONE {
                assert_eq!(self.prev[b as usize], prev);
                assert_eq!(self.order[b as usize], o as u8);
                assert_eq!(b as usize % (1 << o), 0, "misaligned block");
                for p in b as usize..b as usize + (1 << o) {
                    assert!(!covered[p], "page {p} is in two free blocks");
                    covered[p] = true;
                }
                free += 1 << o;
                prev = b;
                b = self.next[b as usize];
            }
        }
        for (o, c) in self.cache.iter().enumerate() {
            for &b in c {
                for p in b as usize..b as usize + (1 << o) {
                    assert!(!covered[p], "cached page {p} is also free");
                    covered[p] = true;
                }
            }
        }
        assert_eq!(free + self.cached_pages(), self.free_pages);
    }
}

impl Core for BuddyCore {
    fn unit(&self) -> usize {
        self.page
    }

    fn total_units(&self) -> usize {
        self.total
    }

    fn free_units(&self) -> usize {
        self.free_pages
    }

    fn alloc(&mut self, units: usize, out: &mut Runs) {
        debug_assert!(units > 0 && units <= self.free_pages);
        self.free_pages -= units;

        let k = ceil_log2(units);
        // Hottest path: an exact block from the cache.
        if units.is_power_of_two() {
            if let Some(b) = self.cache[k as usize].pop() {
                out.push(Run { start: b, len: units as u32 });
                return;
            }
        }

        // Fast path: one contiguous run of exactly `units` pages.
        if k <= self.max_order {
            if let Some(b) = self.exact(units, k) {
                out.push(Run { start: b, len: units as u32 });
                return;
            }
            if self.cached_pages() > 0 {
                self.flush();
                if let Some(b) = self.exact(units, k) {
                    out.push(Run { start: b, len: units as u32 });
                    return;
                }
            }
        } else if self.cached_pages() > 0 {
            self.flush();
        }

        // Fragmented pool: gather the largest blocks that fit, adjacent ones coalesce.
        let mut left = units;
        while left > 0 {
            let want = floor_log2(left).min(self.max_order);
            let fitting = self.nonempty & ((2u64 << want) - 1) as u32;
            let o = if fitting != 0 { 31 - fitting.leading_zeros() } else { want };
            let b = self.alloc_block(o).expect("free page accounting is broken");
            out.push_coalesce(Run { start: b, len: 1 << o });
            left -= 1 << o;
        }
    }

    fn free(&mut self, runs: &[Run]) {
        for run in runs {
            self.free_pages += run.len as usize;
            let len = run.len as usize;
            if len.is_power_of_two() && run.start as usize % len == 0 {
                let o = len.trailing_zeros() as usize;
                if self.cache[o].len() < self.cache_cap {
                    self.cache[o].push(run.start);
                    continue;
                }
            }
            self.free_range(run.start as usize, run.start as usize + len);
        }
    }
}

impl BuddyCore {
    /// Takes an aligned `2^k` block and returns the tail beyond `units` pages.
    #[inline]
    fn exact(&mut self, units: usize, k: u32) -> Option<u32> {
        let b = self.alloc_block(k)? as usize;
        if (1 << k) > units {
            self.free_range(b + units, b + (1 << k));
        }
        Some(b as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn init_covers_everything() {
        for total in [1, 2, 3, 5, 7, 8, 100, 1000, 4096, 4097] {
            let c = BuddyCore::new(4096, total);
            c.check();
            assert_eq!(c.free_units(), total);
        }
    }

    #[test]
    fn exact_contiguous_and_merge_back() {
        let mut c = BuddyCore::new(4096, 1024);
        let mut runs = Runs::new();
        c.alloc(3, &mut runs);
        assert_eq!(runs.as_slice(), &[Run { start: 0, len: 3 }]);
        c.check();
        assert_eq!(c.free_units(), 1021);
        c.free(runs.as_slice());
        c.check();
        assert_eq!(c.histogram()[10], 1, "everything merged back into one block");
    }

    #[test]
    fn random_alloc_free_keeps_invariants() {
        random_case(0);
        random_case(CACHE_CAP);
        random_case(4);
    }

    fn random_case(cache: usize) {
        let total = 3000;
        let mut c = BuddyCore::with_cache(4096, total, cache);
        let mut rng = Rng(0x9E3779B97F4A7C15);
        let mut live: Vec<Runs> = Vec::new();
        let mut owner = vec![usize::MAX; total];
        for step in 0..20_000 {
            let r = rng.next();
            if live.is_empty() || r % 3 != 0 {
                let units =
                    if r % 5 == 0 { 1 << (rng.next() % 7) } else { 1 + (rng.next() % 70) as usize };
                if units > c.free_units() {
                    continue;
                }
                let mut runs = Runs::new();
                c.alloc(units, &mut runs);
                assert_eq!(runs.units(), units);
                for run in runs.as_slice() {
                    for p in run.start..run.start + run.len {
                        assert_eq!(owner[p as usize], usize::MAX, "page {p} handed out twice");
                        owner[p as usize] = step;
                    }
                }
                live.push(runs);
            } else {
                let idx = (rng.next() as usize) % live.len();
                let runs = live.swap_remove(idx);
                for run in runs.as_slice() {
                    for p in run.start..run.start + run.len {
                        owner[p as usize] = usize::MAX;
                    }
                }
                c.free(runs.as_slice());
            }
            if step % 997 == 0 {
                c.check();
            }
        }
        for runs in live.drain(..) {
            c.free(runs.as_slice());
        }
        c.check();
        assert_eq!(c.free_units(), total);
        c.flush();
        c.check();
        // 3000 = 2048 + 512 + 256 + 128 + 32 + 16 + 8: fully merged again.
        assert_eq!(c.histogram().iter().sum::<usize>(), 7);
    }
}
