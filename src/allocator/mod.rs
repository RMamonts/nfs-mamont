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
