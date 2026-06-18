use std::num::NonZeroUsize;

use crate::allocator::Allocator;
use crate::mocks::buffer::MockBuffers;

pub struct MockAllocator {
    block_size: usize,
}

impl MockAllocator {
    pub fn new(block_size: usize) -> Self {
        Self { block_size }
    }

    pub fn empty() -> Self {
        Self { block_size: 0 }
    }
}

impl Allocator for MockAllocator {
    type Buffer = MockBuffers;

    async fn allocate(&self, size: NonZeroUsize) -> Option<MockBuffers> {
        if self.block_size == 0 {
            return None;
        }
        let mut actual_size = 0;
        let mut collector = Vec::new();
        while actual_size < size.get() {
            let buf = vec![0; self.block_size].into_boxed_slice();
            actual_size += buf.len();
            collector.push(buf);
        }
        Some(MockBuffers::new(collector, size.get()))
    }
}
