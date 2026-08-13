use std::num::NonZeroUsize;
use std::sync::Mutex;

use crate::allocator::{Allocator, Slice, UnownedBuffer};

/// Mock allocator for parser tests.
///
/// Buffers are backed by leaked `Box<[u8]>` slices; `allocate_block` always
/// yields a single block of `block_size` bytes, exactly like the real
/// allocator, which means the block can be *larger* than what was requested.
/// An optional per-allocation `limit` caps the size of `try_allocate` results,
/// emulating a pool that can only satisfy part of the request.
pub struct MockAllocator {
    max_size: usize,
    block_size: usize,
    limit: Option<usize>,
    /// Byte the blocks are pre-filled with, standing in for the leftovers of a
    /// previous request in a recycled pool block.
    stale: u8,
    _backing: Mutex<Vec<Box<[u8]>>>,
}

impl MockAllocator {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            block_size: max_size.max(1),
            limit: None,
            stale: 0,
            _backing: Mutex::new(Vec::new()),
        }
    }

    /// Creates an allocator whose `try_allocate` returns buffers capped at
    /// `limit` bytes even when more was requested.
    pub fn partial(max_size: usize, limit: usize) -> Self {
        Self { limit: Some(limit), ..Self::new(max_size) }
    }

    /// Creates an allocator that never satisfies `try_allocate` and hands out
    /// `block_size`-byte blocks pre-filled with `stale`, reproducing a pool
    /// whose recycled blocks still hold another request's bytes.
    pub fn stale_blocks(max_size: usize, block_size: usize, stale: u8) -> Self {
        Self { block_size, limit: Some(0), stale, ..Self::new(max_size) }
    }

    fn make_slice(&self, len: usize, range_end: usize) -> Slice {
        let buf = vec![self.stale; len].into_boxed_slice();
        let ptr = buf.as_ptr() as *mut u8;
        self._backing.lock().unwrap().push(buf);
        let buffer = unsafe { UnownedBuffer::from_raw_parts(ptr, len) };
        Slice::new(vec![buffer], 0..range_end, None)
    }
}

impl Allocator for MockAllocator {
    type Buffer = Slice;

    fn try_allocate(&self, size: NonZeroUsize) -> Option<Slice> {
        if size.get() > self.max_size {
            return None;
        }
        let len = match self.limit {
            Some(limit) => limit.min(size.get()),
            None => size.get(),
        };
        // No memory at all is a failed attempt, not an empty buffer.
        Some(self.make_slice(NonZeroUsize::new(len)?.get(), len))
    }

    fn allocate_block(&self) -> impl std::future::Future<Output = Slice> + Send {
        let slice = self.make_slice(self.block_size, self.block_size);
        async move { slice }
    }

    /// `max_size == 0` means "`try_allocate` always fails"; it cannot be
    /// expressed as a [`NonZeroUsize`], so such a mock reports a capacity of
    /// one byte. Callers still fall back to [`Self::allocate_block`], which
    /// always succeeds --- just as the real allocator does.
    fn capacity(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.max_size).unwrap_or(NonZeroUsize::MIN)
    }
}
