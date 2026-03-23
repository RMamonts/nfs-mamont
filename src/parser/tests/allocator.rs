use std::num::NonZeroUsize;

use tokio::sync::mpsc;

use crate::allocator::multilevel::alloc::MultiAllocator;
use crate::allocator::multilevel::slice::MultiSlice;
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
    async fn allocate(&mut self, size: NonZeroUsize) -> Option<Slice> {
        if size.get() <= self.max_size {
            let (sender, _) = mpsc::unbounded_channel::<Box<[u8]>>();
            Some(Slice::new(vec![vec![0; size.get()].into_boxed_slice()], 0..size.get(), sender))
        } else {
            None
        }
    }
}

impl MultiAllocator for MockAllocator {
    async fn allocate_multi(&mut self, size: NonZeroUsize) -> Option<MultiSlice> {
        if size.get() <= self.max_size {
            let (sender, _) = mpsc::unbounded_channel::<Box<[u8]>>();
            Some(MultiSlice::new(
                vec![vec![0; size.get()].into_boxed_slice()],
                0..size.get(),
                sender,
                None,
            ))
        } else {
            None
        }
    }
}
