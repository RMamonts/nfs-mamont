mod slice;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::Arc;

use async_channel;
use crossbeam_queue::ArrayQueue;

pub use slice::Slice;

type Buffer = Box<[u8]>;

struct Semaphore {
    sender: async_channel::Sender<()>,
    receiver: async_channel::Receiver<()>,
}

impl std::fmt::Debug for Semaphore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Semaphore").finish()
    }
}

impl Semaphore {
    fn new(permits: usize) -> Self {
        let (sender, receiver) = async_channel::bounded(permits);
        for _ in 0..permits {
            sender.try_send(()).ok();
        }
        Self { sender, receiver }
    }

    async fn acquire_many(&self, n: u32) -> Result<Permit, ()> {
        for _ in 0..n {
            self.receiver.recv().await.map_err(|_| ())?;
        }
        Ok(Permit { n, sender: self.sender.clone() })
    }

    fn add_permits(&self, n: usize) {
        for _ in 0..n {
            let _ = self.sender.try_send(());
        }
    }
}

struct Permit {
    n: u32,
    sender: async_channel::Sender<()>,
}

impl Permit {
    fn forget(self) {
        std::mem::forget(self);
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        for _ in 0..self.n {
            let _ = self.sender.try_send(());
        }
    }
}

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
