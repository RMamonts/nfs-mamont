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

#[derive(Debug)]
pub struct AlignedBuffer {
    ptr: *mut u8,
    len: usize,
}
unsafe impl Send for AlignedBuffer {}
unsafe impl Sync for AlignedBuffer {}

impl AlignedBuffer {
    pub fn new(size: usize) -> Self {
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            panic!("mmap failed");
        }
        // Prefault pages to avoid page faults on I/O
        unsafe { std::ptr::write_bytes(ptr, 0, size); }
        Self { ptr: ptr as *mut u8, len: size }
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr as *mut libc::c_void, self.len);
        }
    }
}

impl std::ops::Deref for AlignedBuffer {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}
impl std::ops::DerefMut for AlignedBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

pub type Buffer = AlignedBuffer;

/// Shared state of the allocator to allow return of buffers and permit restoration.
#[derive(Debug)]
pub struct AllocatorState {
    pub pool: ArrayQueue<Buffer>,
    pub semaphore: Semaphore,
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

        for _ in 0..count.get() {
            pool.push(AlignedBuffer::new(size.get())).expect("can't initialize allocator");
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
