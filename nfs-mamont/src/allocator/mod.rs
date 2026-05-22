//! Defines [`Allocator`] interface used to bound allocation of buffers
//! for user data transmission inside NFS-Mamont implementation.

mod slice;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use tokio::sync::Semaphore;

pub use slice::Slice;

type PoolBuffer = Box<[u8]>;

/// Shared state of the allocator to allow return of buffers and permit restoration.
#[derive(Debug)]
pub struct AllocatorState {
    pub pool: ArrayQueue<PoolBuffer>,
    pub semaphore: Semaphore,
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
    /// # Parameters
    ///
    /// - `size` --- minimum size of the returned buffer in bytes.
    ///
    /// # Panic
    ///
    /// This method returns [`None`] if size is greater then allocator capacity.
    fn allocate(&self, size: NonZeroUsize) -> impl Future<Output = Option<Self::Buffer>> + Send;
}

pub struct Impl {
    state: Arc<AllocatorState>,
    buffer_size: NonZeroUsize,
    buffer_count: NonZeroUsize,
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

        for _ in 0..count.get() {
            pool.push(vec![0; size.get()].into_boxed_slice()).expect("can't initialize allocator");
        }

        Self {
            state: Arc::new(AllocatorState { pool, semaphore }),
            buffer_size: size,
            buffer_count: count,
        }
    }

    fn capacity(&self) -> usize {
        self.buffer_size.get() * self.buffer_count.get()
    }
}

impl Allocator for Impl {
    type Buffer = slice::Slice;

    async fn allocate(&self, size: NonZeroUsize) -> Option<Self::Buffer> {
        if size.get() > self.capacity() {
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
}
