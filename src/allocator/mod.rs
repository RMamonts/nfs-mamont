//! Defines [`Allocator`] interface used to bound allocation of buffers
//! for user data transmission inside NFS-Mamont implementation.

mod slice;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};

pub use slice::Slice;

type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;
type Buffer = Box<[u8]>;

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
    fn allocate(&mut self, size: NonZeroUsize)
        -> impl Future<Output = Option<slice::Slice>> + Send;
}

/// A cheap-clone handle that shares one allocator instance across tasks.
///
/// This is useful when the server needs a single global buffer pool and
/// per-connection parsers should all draw from the same resource limit.
pub struct SharedAllocator<A>(Arc<Mutex<A>>);

impl<A> SharedAllocator<A> {
    /// Wraps an allocator so it can be cloned and shared across tasks.
    pub fn new(allocator: A) -> Self {
        Self(Arc::new(Mutex::new(allocator)))
    }
}

impl<A> Clone for SharedAllocator<A> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<A> From<A> for SharedAllocator<A> {
    fn from(value: A) -> Self {
        Self::new(value)
    }
}

impl<A: Allocator + Send> Allocator for SharedAllocator<A> {
    async fn allocate(&mut self, size: NonZeroUsize) -> Option<slice::Slice> {
        let mut allocator = self.0.lock().await;
        allocator.allocate(size).await
    }
}

pub struct Impl {
    receiver: Receiver<Buffer>,
    sender: Sender<Buffer>,
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
        let (sender, receiver) = mpsc::unbounded_channel::<Buffer>();

        for _ in 0..count.get() {
            sender
                .send(vec![0; size.get()].into_boxed_slice())
                .expect("can't initialize allocator");
        }

        Self { sender, receiver, buffer_size: size, buffer_count: count }
    }

    fn capacity(&self) -> usize {
        self.buffer_size.get() * self.buffer_count.get()
    }
}

impl Allocator for Impl {
    async fn allocate(&mut self, size: NonZeroUsize) -> Option<slice::Slice> {
        if size.get() > self.capacity() {
            return None;
        }

        let mut remain_size = size.get();
        let mut buffers = Vec::with_capacity(remain_size.div_ceil(self.buffer_size.get()));

        while remain_size > 0 {
            let buffer = self.receiver.recv().await?;
            assert_eq!(buffer.len(), self.buffer_size.get());

            remain_size = remain_size.saturating_sub(buffer.len());
            buffers.push(buffer);
        }

        Some(Slice::new(buffers, 0..size.get(), self.sender.clone()))
    }
}
