use std::num::NonZeroUsize;
use std::sync::Mutex;

use crate::allocator::{Allocator, Slice, UnownedBuffer};

/// Mock allocator for parser tests.
///
/// Buffers are backed by leaked `Box<[u8]>` slices; `allocate_block` always
/// yields a single block of `max_size` bytes. An optional per-allocation
/// `limit` caps the size of `try_allocate` results, emulating a pool that can
/// only satisfy part of the request.
pub struct MockAllocator {
    max_size: usize,
    limit: Option<usize>,
    _backing: Mutex<Vec<Box<[u8]>>>,
}

impl MockAllocator {
    pub fn new(max_size: usize) -> Self {
        Self { max_size, limit: None, _backing: Mutex::new(Vec::new()) }
    }

    /// Creates an allocator whose `try_allocate` returns buffers capped at
    /// `limit` bytes even when more was requested.
    pub fn partial(max_size: usize, limit: usize) -> Self {
        Self { max_size, limit: Some(limit), _backing: Mutex::new(Vec::new()) }
    }

    fn make_slice(&self, len: usize, range_end: usize) -> Slice {
        let buf = vec![0; len].into_boxed_slice();
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
        Some(self.make_slice(len, len))
    }

    fn allocate_block(&self) -> impl std::future::Future<Output = Slice> + Send {
        let len = self.max_size.max(1);
        let slice = self.make_slice(len, len);
        async move { slice }
    }

    /// `max_size == 0` means "every allocation fails"; it cannot be expressed as
    /// a [`NonZeroUsize`], so such a mock reports a capacity of one byte that
    /// [`Self::try_allocate`] still refuses.
    fn capacity(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.max_size).unwrap_or(NonZeroUsize::MIN)
    }
}
