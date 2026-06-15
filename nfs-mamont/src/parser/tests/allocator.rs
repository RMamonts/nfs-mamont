use std::num::NonZeroUsize;

use tokio::sync::mpsc;

use crate::allocator::{Allocator, Slice, UnownedBuffer};

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
            let (_, _) = mpsc::unbounded_channel::<Box<[u8]>>();
            let buf = vec![0; size.get()].into_boxed_slice();
            let len = buf.len();
            let ptr = Box::into_raw(buf) as *mut u8;
            let buffer = unsafe { UnownedBuffer::from_raw_parts(ptr, len) };
            Some(Slice::new(vec![buffer], 0..size.get(), None))
        } else {
            None
        }
    }
}
