use std::num::NonZeroUsize;

use crate::allocator::mock::buffer::MockBuffers;
use crate::allocator::mock::buffer::{MAX_BLOCK_AMOUNT, MAX_BLOCK_SIZE};
use crate::allocator::Allocator;

pub struct MockAllocator {
    block_size: usize,
}

impl MockAllocator {
    pub fn new(block_size: usize) -> Self {
        Self {
            // Cap block size to prevent OOM in fuzzing/tests
            block_size: block_size.min(MAX_BLOCK_SIZE),
        }
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

        // Cap allocation to max test size to prevent OOM
        let capped_size = size.get().min(MAX_BLOCK_AMOUNT * MAX_BLOCK_SIZE);

        let mut actual_size = 0;
        let mut collector = Vec::new();
        while actual_size < capped_size {
            let remaining = capped_size - actual_size;
            let buf_size = self.block_size.min(remaining);
            let buf = vec![0; buf_size].into_boxed_slice();
            actual_size += buf.len();
            collector.push(buf);
        }
        Some(MockBuffers::new(collector, capped_size))
    }

    fn capacity(&self) -> NonZeroUsize {
        // Return capped capacity to prevent parser from trying to allocate too much
        NonZeroUsize::new(MAX_BLOCK_AMOUNT * MAX_BLOCK_SIZE).expect("non-zero")
    }
}
