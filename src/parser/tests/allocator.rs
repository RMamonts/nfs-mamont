use crate::allocator::{Allocator, Slice};
use async_trait::async_trait;
use std::num::NonZeroUsize;
use tokio::sync::mpsc;

pub struct MockAllocator {
    max_size: usize,
}

impl MockAllocator {
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }
}

#[async_trait]
impl Allocator for MockAllocator {
    async fn alloc(&mut self, size: NonZeroUsize) -> Option<Slice> {
        if size.get() <= self.max_size {
            let (sender, _) = mpsc::unbounded_channel::<Box<[u8]>>();
            Some(Slice::new(vec![vec![0; size.get()].into_boxed_slice()], 0..size.get(), sender))
        } else {
            None
        }
    }
}
