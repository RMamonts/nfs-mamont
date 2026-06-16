use std::ops::{Deref, DerefMut};

use crate::allocator::{Slice, UnownedBuffer};

pub struct SliceGuard {
    slice: Slice,
    _backing: Vec<Box<[u8]>>,
}

impl SliceGuard {
    pub fn new<Buffers>(buffers: Buffers, range: std::ops::Range<usize>) -> Self
    where
        Buffers: IntoIterator<IntoIter: ExactSizeIterator<Item = &'static [u8]>>,
    {
        let mut backing = Vec::new();
        let mut unowned = Vec::new();
        for buf in buffers {
            let boxed = buf.to_vec().into_boxed_slice();
            let ptr = boxed.as_ptr() as *mut u8;
            let len = boxed.len();
            unowned.push(unsafe { UnownedBuffer::from_raw_parts(ptr, len) });
            backing.push(boxed);
        }
        Self { slice: Slice::new(unowned, range, None), _backing: backing }
    }
}

impl Deref for SliceGuard {
    type Target = Slice;

    fn deref(&self) -> &Slice {
        &self.slice
    }
}

impl DerefMut for SliceGuard {
    fn deref_mut(&mut self) -> &mut Slice {
        &mut self.slice
    }
}
