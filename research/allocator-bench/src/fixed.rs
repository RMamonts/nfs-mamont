//! Fixed-size blocks with a LIFO free list: the upstream design (one chunk per
//! block, `ceil(size / block)` blocks per request) with cheaper bookkeeping and
//! hot-buffer reuse.

use crate::pool::Core;
use crate::runs::{Run, Runs};

pub struct FixedCore {
    unit: usize,
    total: usize,
    /// LIFO stack of free block indices; the top is the most recently freed block.
    free: Vec<u32>,
}

impl FixedCore {
    pub fn new(unit: usize, count: usize) -> Self {
        assert!(unit > 0 && count > 0 && count < u32::MAX as usize);
        // Reverse order: the first allocation gets block 0, 1, 2, ...
        Self { unit, total: count, free: (0..count as u32).rev().collect() }
    }
}

impl Core for FixedCore {
    fn unit(&self) -> usize {
        self.unit
    }

    fn total_units(&self) -> usize {
        self.total
    }

    fn free_units(&self) -> usize {
        self.free.len()
    }

    fn alloc(&mut self, units: usize, out: &mut Runs) {
        let start = self.free.len() - units;
        for &idx in self.free[start..].iter().rev() {
            out.push(Run { start: idx, len: 1 });
        }
        self.free.truncate(start);
    }

    fn free(&mut self, runs: &[Run]) {
        // Push in reverse so that the next allocation pops the blocks back in the
        // same order (keeps hot blocks hot).
        for run in runs.iter().rev() {
            debug_assert_eq!(run.len, 1);
            self.free.push(run.start);
        }
    }
}

/// Fixed-size blocks recycled FIFO, like upstream's `ArrayQueue` (control variant used
/// to separate the effect of reuse order from everything else).
pub struct FifoCore {
    unit: usize,
    total: usize,
    free: std::collections::VecDeque<u32>,
}

impl FifoCore {
    pub fn new(unit: usize, count: usize) -> Self {
        assert!(unit > 0 && count > 0 && count < u32::MAX as usize);
        Self { unit, total: count, free: (0..count as u32).collect() }
    }
}

impl Core for FifoCore {
    fn unit(&self) -> usize {
        self.unit
    }

    fn total_units(&self) -> usize {
        self.total
    }

    fn free_units(&self) -> usize {
        self.free.len()
    }

    fn alloc(&mut self, units: usize, out: &mut Runs) {
        for _ in 0..units {
            out.push(Run { start: self.free.pop_front().unwrap(), len: 1 });
        }
    }

    fn free(&mut self, runs: &[Run]) {
        for run in runs {
            self.free.push_back(run.start);
        }
    }
}
