//! Defines [`Allocator`] interface used to bound allocation of buffers
//! for user data transmission inside NFS-Mamont implementation.

mod slice;

#[cfg(test)]
mod tests;

use std::num::NonZeroUsize;

use async_trait::async_trait;
use tokio::sync::mpsc;

pub use slice::Slice;

type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;
type Buffer = Box<[u8]>;

/// Allocates [`Slice`]'s.
#[async_trait]
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
    async fn allocate(&mut self, size: NonZeroUsize) -> Option<slice::Slice>;
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
}

#[async_trait]
impl Allocator for Impl {
    async fn allocate(&mut self, size: NonZeroUsize) -> Option<slice::Slice> {
        assert!(
            size.get() <= self.buffer_size.get() * self.buffer_count.get(),
            "cannot allocate more than allocattor capacity"
        );

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
