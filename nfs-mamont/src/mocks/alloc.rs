use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::Mutex;

use crate::allocator::Allocator;
use crate::mocks::buffer::MockBuffers;

pub struct MockAllocator {
    max_size: usize,
    _backing: Mutex<VecDeque<Box<[u8]>>>,
}

impl MockAllocator {
    pub fn new(block_amounts: usize, block_size: usize) -> Self {
        let mut bufs = VecDeque::with_capacity(block_amounts);
        for _ in 0..block_amounts {
            let buf = vec![0; block_size].into_boxed_slice();
            bufs.push_back(buf);
        }
        let max_size = block_amounts.checked_mul(block_size).expect("size overflow");
        Self { max_size, _backing: Mutex::new(bufs) }
    }

    pub fn empty() -> Self {
        Self { max_size: 0, _backing: Mutex::new(VecDeque::new()) }
    }
}

impl Allocator for MockAllocator {
    type Buffer = MockBuffers;

    async fn allocate(&self, size: NonZeroUsize) -> Option<MockBuffers> {
        if size.get() <= self.max_size {
            let mut guard = self._backing.lock().unwrap();
            let mut actual_size = 0;
            let mut collector = Vec::new();
            while actual_size < size.get() {
                let next = guard.pop_front()?;
                actual_size += next.len();
                collector.push(next);
            }
            Some(MockBuffers::new(collector, size.get()))
        } else {
            None
        }
    }
}
