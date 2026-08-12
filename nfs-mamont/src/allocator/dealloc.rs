use std::sync::Arc;

use crate::allocator::{buffer::RawBuffer, AllocatorState};

pub trait Deallocator<R: RawBuffer> {
    fn deallocate(&self, item: R);
}

impl<R: RawBuffer> Deallocator<R> for Arc<AllocatorState<R>> {
    fn deallocate(&self, item: R) {
        let _ = self.pool.push(item);
        self.semaphore.add_permits(1);
    }
}
