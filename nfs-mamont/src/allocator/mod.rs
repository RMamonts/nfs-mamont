//! Defines [`Allocator`] interface used to bound allocation of buffers
//! for user data transmission inside NFS-Mamont implementation, and
//! [`PoolAllocator`] --- its built-in pool-based implementation.

mod buffer;
mod slice;

#[cfg(test)]
mod tests;

use std::alloc::{self, Layout};
use std::future::Future;
#[cfg(feature = "mlock")]
use std::io;
use std::num::NonZeroUsize;
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use tokio::sync::Semaphore;

pub use buffer::UnownedBuffer;
pub use slice::Slice;

/// Shared state of the [`PoolAllocator`] to allow return of buffers and permit restoration.
#[derive(Debug)]
pub struct PoolState {
    pub pool: ArrayQueue<UnownedBuffer>,
    pub semaphore: Semaphore,
    base_ptr: *mut u8,
    layout: Layout,
}

unsafe impl Send for PoolState {}
unsafe impl Sync for PoolState {}

impl Drop for PoolState {
    fn drop(&mut self) {
        while self.pool.pop().is_some() {}
        #[cfg(feature = "mlock")]
        unsafe {
            libc::munlock(self.base_ptr as *mut libc::c_void, self.layout.size());
        }
        unsafe { alloc::dealloc(self.base_ptr, self.layout) };
    }
}

/// Abstract buffer type returned by [`Allocator`].
///
/// Implementations provide chunked read/write access to the allocated memory.
pub trait Buffer: Send + Sync {
    /// Returns an iterator over read-only byte chunks of this buffer.
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_;

    /// Returns an iterator over mutable byte chunks of this buffer.
    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_;

    /// Returns the total number of bytes in the buffer.
    fn len(&self) -> usize;

    /// Returns `true` if the buffer is empty.
    fn is_empty(&self) -> bool;

    /// Creates an empty, zero-length buffer with no backing memory.
    fn empty() -> Self
    where
        Self: Sized;
}

/// Allocates buffers for user data transmission inside NFS-Mamont implementation.
pub trait Allocator {
    /// Type of buffer returned by this allocator.
    type Buffer: Buffer;

    /// Returns a buffer of at least `size` bytes.
    ///
    /// Waits until enough memory is available, so the returned future may stay
    /// pending while the pool is exhausted.
    ///
    /// # Parameters
    ///
    /// - `size` --- minimum size of the returned buffer in bytes.
    ///
    /// # Returns
    ///
    /// Returns [`None`] if `size` is greater than [`Allocator::capacity`],
    /// since such a request can never be satisfied.
    fn allocate(&self, size: NonZeroUsize) -> impl Future<Output = Option<Self::Buffer>> + Send;

    /// Returns the largest size [`Allocator::allocate`] can ever satisfy.
    ///
    /// Callers that are free to shorten their request (for example NFSv3 `READ`,
    /// where a short read is legal) should clamp to this value instead of
    /// failing on an oversized request.
    fn capacity(&self) -> NonZeroUsize;
}

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
            state: Arc::new(PoolState { pool, semaphore, base_ptr, layout }),
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
            return None;
        }

        let remain_size = size.get();
        let count_needed = remain_size.div_ceil(self.buffer_size.get());

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

        Some(Slice::new(buffers, 0..size.get(), Some(Arc::clone(&self.state))))
    }

    fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }
}
