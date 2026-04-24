//! Defines [`Allocator`] interface used to bound allocation of buffers
//! for user data transmission inside NFS-Mamont implementation.

mod slice;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::Arc;

use async_channel::{Receiver, Sender};
use crossbeam_queue::ArrayQueue;

pub use slice::Slice;

type Buffer = Box<[u8]>;

/// Shared state of the allocator to allow return of buffers and permit restoration.
#[derive(Debug)]
pub struct AllocatorState {
    pub pool: ArrayQueue<Buffer>,
    pub permits_tx: Sender<()>,
    pub permits_rx: Receiver<()>,
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
        let (permits_tx, permits_rx) = async_channel::bounded::<()>(count.get());

        for _ in 0..count.get() {
            pool.push(vec![0; size.get()].into_boxed_slice()).expect("can't initialize allocator");
            permits_tx.try_send(()).expect("can't initialize allocator permits channel");
        }

        Self {
            state: Arc::new(AllocatorState { pool, permits_tx, permits_rx }),
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

        for _ in 0..count_needed {
            if self.state.permits_rx.recv().await.is_err() {
                return None;
            }
        }

        let mut buffers = Vec::with_capacity(count_needed);
        for _ in 0..count_needed {
            if let Some(buf) = self.state.pool.pop() {
                buffers.push(buf);
            } else {
                unreachable!("Semaphore permitted allocation but pool was empty");
            }
        }

        Some(Slice::new(buffers, 0..size.get(), Some(Arc::clone(&self.state))))
    }
}
