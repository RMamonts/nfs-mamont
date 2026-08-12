use std::alloc::{self, Layout};
#[cfg(feature = "mlock")]
use std::io;
use std::num::NonZeroUsize;
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use tokio::sync::Semaphore;

use crate::allocator::buffer::dropbuf::UnownedDroppableBuffer;
use crate::allocator::slice::nodropslice::SliceNoDrop;
use crate::allocator::AllocatorState;
use crate::Allocator;

#[allow(dead_code)]
pub struct Impl2 {
    state: Arc<AllocatorState<UnownedDroppableBuffer>>,
    buffer_size: NonZeroUsize,
    buffer_count: NonZeroUsize,
}

#[allow(dead_code)]
impl Impl2 {
    pub fn new(size: NonZeroUsize, count: NonZeroUsize) -> Self {
        let pool = ArrayQueue::new(count.get());
        let semaphore = Semaphore::new(count.get());

        let buffer_size = size.get();
        let buffer_count = count.get();

        let total_size = buffer_size.checked_mul(buffer_count).expect("size overflow");
        let layout = Layout::from_size_align(total_size, std::mem::align_of::<u8>())
            .expect("invalid layout");

        let base_ptr = unsafe { alloc::alloc_zeroed(layout) };

        if base_ptr.is_null() {
            alloc::handle_alloc_error(layout);
        }

        #[cfg(feature = "mlock")]
        {
            let ptr = base_ptr as *mut libc::c_void;
            if unsafe { libc::mlock(ptr, total_size) } != 0 {
                let err = io::Error::last_os_error();
                panic!("mlock failed (size={}): {err}", size.get());
            }
        }

        let mut current_ptr = base_ptr;
        let state = Arc::new(AllocatorState { pool, semaphore, base_ptr, layout });
        for _ in 0..buffer_count {
            let buffer = unsafe {
                UnownedDroppableBuffer::from_raw_parts(current_ptr, buffer_size, state.clone())
            };
            state.pool.push(buffer).expect("can't initialize allocator");
            current_ptr = unsafe { current_ptr.add(buffer_size) };
        }

        Self { state, buffer_size: size, buffer_count: count }
    }

    fn capacity(&self) -> usize {
        self.buffer_size.get() * self.buffer_count.get()
    }
}

impl Allocator for Impl2 {
    type Buffer = SliceNoDrop;
    async fn allocate(&self, size: NonZeroUsize) -> Option<Self::Buffer> {
        if size.get() > self.capacity() {
            return None;
        }

        let remain_size = size.get();
        let count_needed = remain_size.div_ceil(self.buffer_size.get());

        let permit = match self.state.semaphore.acquire_many(count_needed as u32).await {
            Ok(p) => p,
            Err(_) => return None,
        };

        let mut buffers = Vec::with_capacity(count_needed);
        for _ in 0..count_needed {
            if let Some(buf) = self.state.pool.pop() {
                buffers.push(buf);
            } else {
                unreachable!("Semaphore permitted allocation but pool was empty");
            }
        }

        permit.forget();

        Some(SliceNoDrop::new(buffers, 0..size.get()))
    }
}
