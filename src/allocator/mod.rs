#![allow(dead_code)]

mod buffer;
mod chain;

use tokio::sync::mpsc;

use buffer::Buffer;
use chain::Chain;

type Sender = mpsc::Sender<buffer::Buffer>;
type Receiver = mpsc::Receiver<buffer::Buffer>;

/// Allocates instrusive linked [`Chain`].
pub struct Allocator {
    receiver: Receiver,
    sender: Sender,
    capacity: usize,
}

impl Allocator {
    /// Crates new allocator with specified buffer size and count.
    ///
    /// # Parameters
    ///
    /// * `size` --- size of each buffer
    /// * `count` --- counts of buffer
    pub async fn new(size: usize, count: usize) -> Self {
        let (sender, receiver) = mpsc::channel::<Buffer>(count);

        for _ in 0..count {
            let buffer = Buffer::alloc(size);
            sender.send(buffer).await.expect("cannot init buffers");
        }

        Self { sender, receiver, capacity: size * count }
    }

    /// Allocates [`Chain`] at least the specified size.
    pub async fn alloc(&mut self, mut size: usize) -> Chain {
        assert!(size < self.capacity);

        let mut chain = Chain::new(self.sender.clone());
        while size > 0 {
            let buffer = self.receiver.recv().await.expect("channel not to be closed");

            size = size.saturating_sub(buffer.len());
            chain.push_head(buffer);
        }

        chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUFFER_SIZE: usize = 32;
    const BUFFER_COUNT: usize = 4;

    #[tokio::test(flavor = "current_thread")]
    async fn alloc_provides_requested_capacity() {
        let mut allocator = Allocator::new(BUFFER_SIZE, BUFFER_COUNT).await;
        let requested = BUFFER_SIZE + 5;

        let mut chain = allocator.alloc(requested).await;
        let allocated = {
            let buffers = Chain::to_vec(&mut chain);
            buffers.iter().map(|buffer| buffer.len()).sum::<usize>()
        };

        assert!(allocated >= requested);
        assert!(allocated <= requested + BUFFER_SIZE);

        Chain::dealloc(chain).await;

        let second = allocator.alloc(BUFFER_SIZE / 2).await;
        Chain::dealloc(second).await;
    }

    #[tokio::test(flavor = "current_thread")]
    #[should_panic]
    async fn alloc_panics_if_request_exceeds_capacity() {
        let mut allocator = Allocator::new(BUFFER_SIZE, BUFFER_COUNT).await;
        allocator.alloc(BUFFER_SIZE * BUFFER_COUNT).await;
    }
}
