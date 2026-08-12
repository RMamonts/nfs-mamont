use std::alloc::{self, Layout};
use std::cmp::min;
use std::num::NonZeroUsize;
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use tokio::sync::Semaphore;

use crate::allocator::buffer::dropbuf::UnownedDroppableBuffer;
use crate::allocator::slice::nodropslice::SliceNoDrop;
use crate::allocator::AllocatorState;
use crate::Allocator;

pub struct SyncImpl {
    state: Arc<AllocatorState<UnownedDroppableBuffer>>,
    buffer_size: NonZeroUsize,
}

impl SyncImpl {
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

        Self { state, buffer_size: size }
    }
}

impl Allocator for SyncImpl {
    type Buffer = SliceNoDrop;
    async fn allocate(&self, size: NonZeroUsize) -> Option<Self::Buffer> {
        // there will defenitely be at least one block
        let block_amount = size.get().div_ceil(self.buffer_size.get());
        let mut counter = 0;
        let mut vec = Vec::with_capacity(block_amount);
        while let Ok(p) = self.state.semaphore.try_acquire() {
            let buf = match self.state.pool.pop() {
                Some(buf) => buf,
                None => unreachable!(),
            };
            vec.push(buf);
            p.forget();
            counter += 1;
        }

        if counter == 0 {
            match self.state.semaphore.acquire().await {
                Ok(p) => {
                    let buf = match self.state.pool.pop() {
                        Some(buf) => buf,
                        None => unreachable!(),
                    };
                    vec.push(buf);
                    p.forget();
                }
                Err(_) => return None,
            }
        }

        let len = vec.iter().map(|s| s.len()).sum();
        let range = 0..min(len, size.get());
        Some(SliceNoDrop::new(vec, range))
    }
}
