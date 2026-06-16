//! Defines [`Allocator`] interface used to bound allocation of buffers
//! for user data transmission inside NFS-Mamont implementation.

mod buffer;
mod slice;

#[cfg(test)]
mod tests;

use std::alloc::{self, Layout};
use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use tokio::sync::Semaphore;

pub use buffer::UnownedBuffer;
pub use slice::Slice;

#[cfg(feature = "arbitrary")]
pub const TEST_SIZE: usize = 5000;
type Buffer = Box<[u8]>;

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
        unsafe { alloc::dealloc(self.base_ptr, self.layout) };
    }
}

/// Allocates [`Slice`]'s.
pub trait Allocator {
    /// Returns [`Slice`] of specified size.
    ///
    /// # Parameters
    ///
    /// - `size` --- size of returned slice.
    ///
    /// # Panic
    ///
    /// This method returns [`None`] if size is greater then allocator capacity.
    fn allocate(&self, size: NonZeroUsize) -> impl Future<Output = Option<slice::Slice>> + Send;
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

        let buffer_size = size.get();
        let buffer_count = count.get();

        let total_size = buffer_size.checked_mul(buffer_count).expect("size overflow");
        let layout = Layout::from_size_align(total_size, std::mem::align_of::<u8>())
            .expect("invalid layout");

        let base_ptr = unsafe { alloc::alloc_zeroed(layout) };
        if base_ptr.is_null() {
            alloc::handle_alloc_error(layout);
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
            buffer_count: count,
        }
    }

    fn capacity(&self) -> usize {
        self.buffer_size.get() * self.buffer_count.get()
    }
}

impl Allocator for Impl {
    async fn allocate(&self, size: NonZeroUsize) -> Option<slice::Slice> {
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
