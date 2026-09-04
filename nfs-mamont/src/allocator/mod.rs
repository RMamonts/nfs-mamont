//! Defines [`Allocator`] interface used to bound allocation of buffers
//! for user data transmission inside NFS-Mamont implementation.

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

/// Shared state of the allocator to allow return of buffers and permit restoration.
#[derive(Debug)]
pub struct AllocatorState {
    pub pool: ArrayQueue<UnownedBuffer>,
    pub semaphore: Semaphore,
    base_ptr: *mut u8,
    layout: Layout,
}

unsafe impl Send for AllocatorState {}
unsafe impl Sync for AllocatorState {}

impl Drop for AllocatorState {
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

pub struct Impl {
    state: Arc<AllocatorState>,
    buffer_size: NonZeroUsize,
    capacity: NonZeroUsize,
}

impl Impl {
    /// Returns new [`Allocator`] IMPlementation.
    ///
    /// # Parameters
    ///
    /// - `size` --- size of each buffer to allocate
    /// - `count` --- number of buffers to allocate
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
            state: Arc::new(AllocatorState { pool, semaphore, base_ptr, layout }),
            buffer_size: size,
            // `total_size` is a product of two non-zero values, checked above.
            capacity: NonZeroUsize::new(total_size).expect("capacity must be non-zero"),
        }
    }
}

/// Number of pool buffers needed to serve a request of `size` bytes.
///
/// Split out of [`Impl::allocate`] so the two facts the rest of the allocator
/// relies on --- the pool is never asked for more buffers than it holds, and the
/// buffers handed back always cover `size` bytes --- can be proved on their own.
fn buffers_needed(size: NonZeroUsize, buffer_size: NonZeroUsize) -> usize {
    size.get().div_ceil(buffer_size.get())
}

impl Allocator for Impl {
    type Buffer = slice::Slice;

    async fn allocate(&self, size: NonZeroUsize) -> Option<Self::Buffer> {
        if size > self.capacity {
            return None;
        }

        let count_needed = buffers_needed(size, self.buffer_size);

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

#[cfg(kani)]
mod verification {
    use super::*;

    /// Bounds on the pool geometry. The interesting cases are all about how a
    /// request divides by the buffer size, so a handful of sizes and a handful of
    /// buffers cover the arithmetic without handing CBMC a 64-bit multiplication
    /// over the whole domain.
    const MAX_BUFFER_SIZE: usize = 8;
    const MAX_BUFFER_COUNT: usize = 4;

    /// [`Impl::allocate`] pops [`buffers_needed`] buffers from the pool and hands
    /// them to [`Slice::new`] as the range `0..size`. Two things must hold for
    /// every request the capacity check lets through, and neither is checked at
    /// runtime: the pool must hold that many buffers (`allocate` otherwise reaches
    /// its `unreachable!`), and the buffers must cover `size` bytes (`Slice::new`
    /// otherwise trips its "cannot index list as slice to end" assert).
    #[kani::proof]
    fn allocate_asks_for_a_covering_number_of_buffers() {
        let buffer_size: usize = kani::any();
        let buffer_count: usize = kani::any();
        kani::assume(buffer_size >= 1 && buffer_size <= MAX_BUFFER_SIZE);
        kani::assume(buffer_count >= 1 && buffer_count <= MAX_BUFFER_COUNT);

        // `Impl::new` computes the capacity this way and panics on overflow.
        let capacity = buffer_size * buffer_count;

        let size: usize = kani::any();
        // Everything `allocate` lets past its `size > self.capacity` check.
        kani::assume(size >= 1 && size <= capacity);

        let needed = buffers_needed(
            NonZeroUsize::new(size).unwrap(),
            NonZeroUsize::new(buffer_size).unwrap(),
        );

        assert!(needed >= 1);
        assert!(needed <= buffer_count);
        assert!(size <= needed * buffer_size);
    }
}
