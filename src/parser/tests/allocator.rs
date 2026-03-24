use std::num::NonZeroUsize;

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
            Some(Slice::new(
                vec![vec![0; size.get()].into_boxed_slice()],
                0..size.get(),
                crate::allocator::detached_sender(),
            ))
        } else {
            None
        }
    }
}
