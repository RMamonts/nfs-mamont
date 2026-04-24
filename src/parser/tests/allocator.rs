use std::num::NonZeroUsize;

use tokio::sync::mpsc;

use crate::allocator::{Allocator, Slice};

pub struct MockAllocator {
    max_size: usize,
}

impl MockAllocator {
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }
}

impl Allocator for MockAllocator {
    async fn allocate(&self, size: NonZeroUsize) -> Option<Slice> {
        if size.get() <= self.max_size {
            let (_, _) = mpsc::unbounded_channel::<crate::allocator::Buffer>();
            Some(Slice::new(vec![crate::allocator::Buffer::new(size.get())], 0..size.get(), None))
        } else {
            None
        }
    }
}
