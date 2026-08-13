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

    /// Shrinks the buffer to at most `len` bytes.
    ///
    /// Buffers handed out by an allocator may be larger than the payload that
    /// was actually written into them (see [`Allocator::allocate_block`]).
    /// Truncating hides the untouched tail --- which still holds bytes of
    /// whatever request used the memory before --- from every later consumer.
    ///
    /// Does nothing when `len` is not smaller than the current length. The
    /// backing memory is kept and released as a whole when the buffer is
    /// dropped.
    fn truncate(&mut self, len: usize);

    /// Creates an empty, zero-length buffer with no backing memory.
    fn empty() -> Self
    where
        Self: Sized;
}

/// Allocates buffers for user data transmission inside NFS-Mamont implementation.
pub trait Allocator {
    /// Type of buffer returned by this allocator.
    type Buffer: Buffer;

    /// Attempts to allocate a buffer of (at most) `size` bytes without blocking.
    ///
    /// This is a "best-effort" operation: the returned buffer may be smaller
    /// than `size` (partial allocation), both when `size` exceeds
    /// [`Self::capacity`] and when part of the pool is currently held by
    /// someone else. If no memory at all can be granted right now, [`None`] is
    /// returned — the caller should either retry later or fall back to the
    /// blocking [`Self::allocate_block`].
    ///
    /// # Parameters
    ///
    /// - `size` --- maximum size of the returned buffer in bytes.
    fn try_allocate(&self, size: NonZeroUsize) -> Option<Self::Buffer>;

    /// Allocates a single minimal buffer, blocking until one is available.
    ///
    /// The returned buffer is exactly one allocation unit of the allocator and
    /// is not guaranteed to satisfy the originally requested `size`. It may
    /// therefore be *larger* than what the caller intends to fill, so a caller
    /// that hands the buffer on must [`Buffer::truncate`] it to the number of
    /// bytes it actually wrote.
    fn allocate_block(&self) -> impl Future<Output = Self::Buffer> + Send;

    /// Returns a buffer of at most `size` bytes.
    ///
    /// This is a convenience combination of [`Self::try_allocate`] and
    /// [`Self::allocate_block`]: it first attempts a non-blocking allocation,
    /// and if the pool is exhausted, falls back to blocking for a single block.
    /// The returned buffer is best-effort: it may be partial (`len() < size`)
    /// when the allocator cannot provide the full size without waiting, and it
    /// may exceed `size` when the fallback block is bigger than the request.
    ///
    /// # Parameters
    ///
    /// - `size` --- requested size of the returned buffer in bytes.
    fn allocate(&self, size: NonZeroUsize) -> impl Future<Output = Self::Buffer> + Send
    where
        Self: Sync,
    {
        async move {
            match self.try_allocate(size) {
                Some(buffer) => buffer,
                None => self.allocate_block().await,
            }
        }
    }

    /// Returns the largest size an allocation can ever be fully satisfied for.
    ///
    /// Callers that are free to shorten their request (for example NFSv3 `READ`,
    /// where a short read is legal) should clamp to this value instead of
    /// failing on an oversized request. Beyond this size only partial
    /// allocations or single blocks are available.
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

impl Allocator for Impl {
    type Buffer = slice::Slice;

    fn try_allocate(&self, size: NonZeroUsize) -> Option<Self::Buffer> {
        // A request beyond the pool size can never be satisfied in full, so it
        // is clamped instead of rejected: a capacity-sized buffer serves the
        // caller far better than the single block it would fall back to.
        let wanted = size.get().min(self.capacity.get());

        // Grant as many blocks as the pool can spare right now. Each failed
        // attempt lowers the request by at least one block, so the loop always
        // terminates, and it drops straight to the observed permit count
        // instead of walking down one block at a time.
        let mut count = wanted.div_ceil(self.buffer_size.get());
        let permit = loop {
            if count == 0 {
                return None;
            }
            match self.state.semaphore.try_acquire_many(count as u32) {
                Ok(permit) => break permit,
                Err(_) => {
                    let available = self.state.semaphore.available_permits();
                    count = count.saturating_sub(1).min(available);
                }
            }
        };

        let mut buffers = Vec::with_capacity(count);
        for _ in 0..count {
            if let Some(buf) = self.state.pool.pop() {
                buffers.push(buf);
            } else {
                unreachable!("Semaphore permitted allocation but pool was empty");
            }
        }

        permit.forget();

        let granted = wanted.min(count * self.buffer_size.get());

        Some(Slice::new(buffers, 0..granted, Some(Arc::clone(&self.state))))
    }

    async fn allocate_block(&self) -> Self::Buffer {
        let permit =
            self.state.semaphore.acquire().await.expect("allocator semaphore is never closed");

        let buf = self.state.pool.pop().expect("Semaphore permitted allocation but pool was empty");

        permit.forget();

        Slice::new(vec![buf], 0..self.buffer_size.get(), Some(Arc::clone(&self.state)))
    }

    fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }
}
