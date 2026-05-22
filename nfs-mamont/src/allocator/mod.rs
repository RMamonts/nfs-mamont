mod slice;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::Arc;

use async_lock::Semaphore;
use crossbeam_queue::ArrayQueue;

pub use slice::Slice;

type Buffer = Box<[u8]>;

#[derive(Debug)]
pub struct AllocatorState {
    pub pool: ArrayQueue<Buffer>,
    pub semaphore: Semaphore,
}

pub trait Allocator {
    fn allocate(&self, size: NonZeroUsize) -> impl Future<Output = Option<slice::Slice>> + Send;
}

pub struct Impl {
    state: Arc<AllocatorState>,
    buffer_size: NonZeroUsize,
    buffer_count: NonZeroUsize,
}

impl Impl {
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
    async fn allocate(&self, size: NonZeroUsize) -> Option<slice::Slice> {
        if size.get() > self.capacity() {
            return None;
        }

        let remain_size = size.get();
        let count_needed = remain_size.div_ceil(self.buffer_size.get());

        let mut guards = Vec::with_capacity(count_needed);
        for _ in 0..count_needed {
            guards.push(self.state.semaphore.acquire().await);
        }

        let mut buffers = Vec::with_capacity(count_needed);
        for _ in 0..count_needed {
            if let Some(buf) = self.state.pool.pop() {
                buffers.push(buf);
            } else {
                unreachable!("Semaphore permitted allocation but pool was empty");
            }
        }

        for guard in guards {
            std::mem::forget(guard);
        }

        Some(Slice::new(buffers, 0..size.get(), Some(Arc::clone(&self.state))))
    }
}
