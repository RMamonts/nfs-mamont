//! Defines [`Allocator`] interface used to bound allocation of buffers
//! for user data transmission inside NFS-Mamont implementation.

mod slice;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub use slice::Slice;

type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;
type Buffer = Box<[u8]>;

#[derive(Clone)]
pub struct MemoryBudget {
    semaphore: Arc<Semaphore>,
}

impl MemoryBudget {
    /// Returns a shared server-wide budget counted in fixed-size buffers.
    pub fn new(buffer_count: NonZeroUsize) -> Self {
        Self { semaphore: Arc::new(Semaphore::new(buffer_count.get())) }
    }

    /// Waits until the requested number of buffers can be charged to the global budget.
    pub async fn acquire_many(&self, buffer_count: usize) -> Option<OwnedSemaphorePermit> {
        let buffer_count = u32::try_from(buffer_count).ok()?;
        self.semaphore.clone().acquire_many_owned(buffer_count).await.ok()
    }

    /// Returns the number of currently available buffer permits.
    pub fn available(&self) -> usize {
        self.semaphore.available_permits()
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
    /// This method panics if size is greater then allocator capacity.
    fn allocate(&mut self, size: NonZeroUsize)
        -> impl Future<Output = Option<slice::Slice>> + Send;
}

pub struct Impl {
    receiver: Receiver<Buffer>,
    sender: Sender<Buffer>,
    buffer_size: NonZeroUsize,
    buffer_count: NonZeroUsize,
    budget: Option<MemoryBudget>,
}

impl Impl {
    /// Returns new [`Allocator`] IMPlementation.
    ///
    /// # Parameters
    ///
    /// - `size` --- size of each buffer to allocate
    /// - `count` --- number of buffers to allocate
    pub fn new(size: NonZeroUsize, count: NonZeroUsize) -> Self {
        Self::with_optional_budget(size, count, None)
    }

    /// Returns new [`Allocator`] implementation bound by a shared server-wide memory budget.
    pub fn with_budget(size: NonZeroUsize, count: NonZeroUsize, budget: MemoryBudget) -> Self {
        Self::with_optional_budget(size, count, Some(budget))
    }

    fn with_optional_budget(
        size: NonZeroUsize,
        count: NonZeroUsize,
        budget: Option<MemoryBudget>,
    ) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel::<Buffer>();

        for _ in 0..count.get() {
            sender
                .send(vec![0; size.get()].into_boxed_slice())
                .expect("can't initialize allocator");
        }

        Self { sender, receiver, buffer_size: size, buffer_count: count, budget }
    }

    fn capacity(&self) -> usize {
        self.buffer_size.get() * self.buffer_count.get()
    }

    fn buffers_for_size(&self, size: usize) -> usize {
        size.div_ceil(self.buffer_size.get())
    }
}

impl Allocator for Impl {
    async fn allocate(&mut self, size: NonZeroUsize) -> Option<slice::Slice> {
        if size.get() > self.capacity() {
            return None;
        }

        let required_buffers = self.buffers_for_size(size.get());
        let permit = match &self.budget {
            Some(budget) => Some(budget.acquire_many(required_buffers).await?),
            None => None,
        };

        let mut remain_size = size.get();
        let mut buffers = Vec::with_capacity(required_buffers);

        while remain_size > 0 {
            let buffer = self.receiver.recv().await?;
            assert_eq!(buffer.len(), self.buffer_size.get());

            remain_size = remain_size.saturating_sub(buffer.len());
            buffers.push(buffer);
        }

        Some(Slice::new_with_permit(
            buffers,
            0..size.get(),
            self.sender.clone(),
            permit,
        ))
    }
}
