#![allow(dead_code)]

mod buffer;
mod chain;

use std::num::NonZeroUsize;

use tokio::sync::mpsc;

use buffer::Buffer;
use chain::List;

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
    pub async fn new(size: NonZeroUsize, count: NonZeroUsize) -> Self {
        let (sender, receiver) = mpsc::channel::<Buffer>(count.get());

        for _ in 0..count.get() {
            let buffer = Buffer::alloc(size);
            sender.send(buffer).await.expect("cannot init buffers");
        }

        Self { sender, receiver, capacity: size.get() * count.get() }
    }

    /// Allocates [`Chain`] at least the specified size.
    pub async fn alloc(&mut self, mut size: usize) -> List {
        assert!(size < self.capacity);

        let mut chain = List::new(self.sender.clone());
        while size > 0 {
            let buffer = self.receiver.recv().await.expect("channel not to be closed");

            size = size.saturating_sub(buffer.len());
            chain.push_head(buffer);
        }

        chain
    }
}
