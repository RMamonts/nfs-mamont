use std::num::NonZeroUsize;
use std::sync::Mutex;

use crate::allocator::{Allocator, Slice, UnownedBuffer};

pub struct MockAllocator {
    max_size: usize,
    _backing: Mutex<Vec<Box<[u8]>>>,
}

impl MockAllocator {
    pub fn new(max_size: usize) -> Self {
        Self { max_size, _backing: Mutex::new(Vec::new()) }
    }
}

impl Allocator for MockAllocator {
    type Buffer = Slice;

    async fn allocate(&self, size: NonZeroUsize) -> Option<Slice> {
        if size.get() <= self.max_size {
            let buf = vec![0; size.get()].into_boxed_slice();
            let len = buf.len();
            let ptr = buf.as_ptr() as *mut u8;
            self._backing.lock().unwrap().push(buf);
            let buffer = unsafe { UnownedBuffer::from_raw_parts(ptr, len) };
            Some(Slice::new(vec![buffer], 0..size.get(), None))
        } else {
            None
        }
    }
}
