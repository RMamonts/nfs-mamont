//! Defines [`Allocator`] interface used to bound allocation of buffers
//! for user data transmission inside NFS-Mamont implementation.

mod slice;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crossbeam::queue::SegQueue;
use tokio::sync::Notify;

pub use slice::Slice;

type Buffer = Box<[u8]>;

#[derive(Debug)]
pub(crate) struct Recycler {
    queue: Arc<SegQueue<Buffer>>,
    available: Arc<AtomicUsize>,
    notify: Arc<Notify>,
}

impl Recycler {
    pub(crate) fn send(&self, buffer: Buffer) {
        self.queue.push(buffer);
        self.available.fetch_add(1, Ordering::Release);
        self.notify.notify_one();
    }
}

type Sender = Arc<Recycler>;

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
    /// This method panics if size is greater then allocator capacity.
    fn allocate(&self, size: NonZeroUsize) -> impl Future<Output = Option<slice::Slice>> + Send;
}

pub struct Impl {
    queue: Arc<SegQueue<Buffer>>,
    available: Arc<AtomicUsize>,
    notify: Arc<Notify>,
    sender: Sender,
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
        let queue = Arc::new(SegQueue::new());
        let available = Arc::new(AtomicUsize::new(count.get()));
        let notify = Arc::new(Notify::new());

        for _ in 0..count.get() {
            queue.push(vec![0; size.get()].into_boxed_slice());
        }

        let sender = Arc::new(Recycler {
            queue: Arc::clone(&queue),
            available: Arc::clone(&available),
            notify: Arc::clone(&notify),
        });

        Self { queue, available, notify, sender, buffer_size: size, buffer_count: count }
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

        let need_buffers = size.get().div_ceil(self.buffer_size.get());

        // Atomically reserve required amount so concurrent allocators do not overcommit.
        loop {
            let available = self.available.load(Ordering::Acquire);

            if available < need_buffers {
                self.notify.notified().await;
                continue;
            }

            if self
                .available
                .compare_exchange_weak(
                    available,
                    available - need_buffers,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                break;
            }
        }

        let mut buffers = Vec::with_capacity(need_buffers);

        for _ in 0..need_buffers {
            loop {
                if let Some(buffer) = self.queue.pop() {
                    assert_eq!(buffer.len(), self.buffer_size.get());
                    buffers.push(buffer);
                    break;
                }

                tokio::task::yield_now().await;
            }
        }

        Some(Slice::new(buffers, 0..size.get(), Arc::clone(&self.sender)))
    }
}

pub(crate) fn detached_sender() -> Sender {
    Arc::new(Recycler {
        queue: Arc::new(SegQueue::new()),
        available: Arc::new(AtomicUsize::new(0)),
        notify: Arc::new(Notify::new()),
    })
}
