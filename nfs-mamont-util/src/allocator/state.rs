//! Defines [`PoolState`] --- memory block and free list shared by
//! [`PoolAllocator`](super::PoolAllocator) and the [`Slice`](super::Slice)s it hands out.

use std::alloc::{self, Layout};

use crossbeam_queue::ArrayQueue;
use tokio::sync::Semaphore;

use super::UnownedBuffer;

/// Shared state of the [`PoolAllocator`](super::PoolAllocator) to allow return of buffers and permit restoration.
#[derive(Debug)]
pub struct PoolState {
    pub pool: ArrayQueue<UnownedBuffer>,
    pub semaphore: Semaphore,
    pub base_ptr: *mut u8,
    pub layout: Layout,
}

unsafe impl Send for PoolState {}
unsafe impl Sync for PoolState {}

impl Drop for PoolState {
    fn drop(&mut self) {
        while self.pool.pop().is_some() {}
        #[cfg(feature = "mlock")]
        unsafe {
            libc::munlock(self.base_ptr as *mut libc::c_void, self.layout.size());
        }
        unsafe { alloc::dealloc(self.base_ptr, self.layout) };
    }
}
