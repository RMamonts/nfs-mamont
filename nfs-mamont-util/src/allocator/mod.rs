//! [`PoolAllocator`] --- pool-based implementation of the
//! [`Allocator`] interface.

mod buffer;
mod slice;
mod state;

#[cfg(test)]
mod tests;

use std::alloc::{self, Layout};
#[cfg(feature = "mlock")]
use std::io;
use std::num::NonZeroUsize;
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use nfs_mamont::Allocator;
use tokio::sync::Semaphore;

use crate::metric::AllocatorMetrics;
use state::PoolState;

use buffer::UnownedBuffer;
pub use slice::Slice;

/// [`Allocator`] backed by a fixed pool of equally sized buffers.
///
/// All buffers are carved out of a single contiguous block allocated up front;
/// [`Allocator::allocate`] hands out as many of them as needed and waits while
/// the pool is exhausted. Buffers return to the pool when the [`Slice`] is dropped.
/// With the `mlock` feature the whole block is locked in RAM.
pub struct PoolAllocator {
    state: Arc<PoolState>,
    buffer_size: NonZeroUsize,
    capacity: NonZeroUsize,
}

impl PoolAllocator {
    /// Creates a pool of `count` buffers of `size` bytes each.
    ///
    /// # Parameters
    ///
    /// - `size` --- size of each buffer in the pool
    /// - `count` --- number of buffers in the pool
    // TODO(artemkhel): multiple allocs would write metrics into the same series
    pub fn new(size: NonZeroUsize, count: NonZeroUsize) -> Self {
        let pool = ArrayQueue::new(count.get());
        let semaphore = Semaphore::new(count.get());

        let buffer_size = size.get();
        let buffer_count = count.get();

        let total_size = buffer_size.checked_mul(buffer_count).expect("size overflow");
        let layout = Layout::from_size_align(total_size, std::mem::align_of::<u8>())
            .expect("invalid layout");

        let base_ptr = unsafe { alloc::alloc_zeroed(layout) };

        if base_ptr.is_null() {
            alloc::handle_alloc_error(layout);
        }

        #[cfg(feature = "mlock")]
        {
            let ptr = base_ptr as *mut libc::c_void;
            if unsafe { libc::mlock(ptr, total_size) } != 0 {
                let err = io::Error::last_os_error();
                panic!("mlock failed (size={}): {err}", size.get());
            }
        }

        let mut current_ptr = base_ptr;
        for _ in 0..buffer_count {
            let buffer = unsafe { UnownedBuffer::from_raw_parts(current_ptr, buffer_size) };
            pool.push(buffer).expect("can't initialize allocator");
            current_ptr = unsafe { current_ptr.add(buffer_size) };
        }

        Self {
            state: Arc::new(PoolState {
                pool,
                semaphore,
                base_ptr,
                layout,
                metrics: AllocatorMetrics::new(buffer_count, size.get()),
            }),
            buffer_size: size,
            // `total_size` is a product of two non-zero values, checked above.
            capacity: NonZeroUsize::new(total_size).expect("capacity must be non-zero"),
        }
    }
}

impl Allocator for PoolAllocator {
    type Buffer = slice::Slice;

    async fn allocate(&self, size: NonZeroUsize) -> Option<Self::Buffer> {
        if size > self.capacity {
            self.state.metrics.rejected();
            return None;
        }

        let remain_size = size.get();
        let count_needed = remain_size.div_ceil(self.buffer_size.get());

        let timer = self.state.metrics.start();
        let permit = match self.state.semaphore.acquire_many(count_needed as u32).await {
            Ok(p) => p,
            Err(_) => return None,
        };

        let mut buffers = Vec::with_capacity(count_needed);
        for _ in 0..count_needed {
            if let Some(buf) = self.state.pool.pop() {
                buffers.push(buf);
            } else {
                unreachable!("Semaphore permitted allocation but pool was empty");
            }
        }

        permit.forget();
        self.state.metrics.allocated(timer, size.get(), count_needed);

        Some(Slice::new(buffers, 0..size.get(), Some(Arc::clone(&self.state))))
    }

    fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }
}
