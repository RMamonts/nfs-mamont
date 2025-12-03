use std::cmp::min;
use std::num::NonZeroUsize;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::allocator::{Allocator, Slice};

const BLOCK: usize = 6;

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
            let mut vec = Vec::new();
            let mut rest = size.get();
            while rest > 0 {
                vec.push(vec![0; min(rest, BLOCK)].into_boxed_slice());
                rest -= min(rest, BLOCK);
            }
            Some(Slice::new(vec, 0..size.get(), sender))
        } else {
            None
        }
    }
}
