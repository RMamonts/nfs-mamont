use std::alloc::{self, Layout};
use std::cmp::min;
use std::num::NonZeroUsize;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use tokio::sync::Semaphore;

use crate::allocator::AllocatorState;
use crate::{Allocator, Buffer};

impl super::buffer::RawBuffer for UnownedDroppableBuffer {}

#[derive(Debug)]
pub struct UnownedDroppableBuffer {
    ptr: *mut u8,
    len: usize,
    state: Arc<AllocatorState<Self>>,
}

unsafe impl Send for UnownedDroppableBuffer {}
unsafe impl Sync for UnownedDroppableBuffer {}

impl UnownedDroppableBuffer {
    /// Creates a new `UnownedBuffer` from raw parts.
    ///
    /// # Safety
    ///
    /// - `ptr` must be valid for reads and writes for `len` bytes.
    /// - `ptr` must be properly aligned for `u8`.
    /// - The caller must ensure that the memory is deallocated exactly once,
    ///   typically by the original allocator that owns the entire block.
    pub unsafe fn from_raw_parts(
        ptr: *mut u8,
        len: usize,
        state: Arc<AllocatorState<Self>>,
    ) -> Self {
        Self { ptr, len, state }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn deallocate(&mut self) {
        let new_buf = unsafe {
            UnownedDroppableBuffer::from_raw_parts(self.ptr, self.len, self.state.clone())
        };
        let _ = self.state.pool.push(new_buf);
        self.state.semaphore.add_permits(1);
    }
}

impl Drop for UnownedDroppableBuffer {
    fn drop(&mut self) {
        self.deallocate();
    }
}

impl Deref for UnownedDroppableBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl DerefMut for UnownedDroppableBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

pub struct SliceNoDrop {
    buffers: Vec<UnownedDroppableBuffer>,
    range: std::ops::Range<usize>,
}

pub struct Iter<'a> {
    slice_iter: std::slice::Iter<'a, UnownedDroppableBuffer>,
    range: std::ops::Range<usize>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let result = self.slice_iter.next()?;

            // save values for current step
            let len = result.len();
            let start = self.range.start;
            let end = self.range.end;

            if start == end {
                return None;
            }

            // make a progress
            self.range.start = self.range.start.saturating_sub(len);
            self.range.end = self.range.end.saturating_sub(len);

            if len > start {
                return Some(&result[start..end.min(len)]);
            }
        }
    }
}

impl<'a> IntoIterator for &'a SliceNoDrop {
    type IntoIter = Iter<'a>;
    type Item = &'a [u8];

    fn into_iter(self) -> Self::IntoIter {
        Iter { slice_iter: self.buffers.iter(), range: self.range.clone() }
    }
}

pub struct IterMut<'a> {
    slice_iter: std::slice::IterMut<'a, UnownedDroppableBuffer>,
    range: std::ops::Range<usize>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut [u8];

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let result = self.slice_iter.next()?;

            // save values for current step
            let len = result.len();
            let start = self.range.start;
            let end = self.range.end;

            if start == end {
                return None;
            }

            // make a progress
            self.range.start = self.range.start.saturating_sub(len);
            self.range.end = self.range.end.saturating_sub(len);

            if len > start {
                return Some(&mut result[start..end.min(len)]);
            }
        }
    }
}

impl<'a> IntoIterator for &'a mut SliceNoDrop {
    type IntoIter = IterMut<'a>;
    type Item = &'a mut [u8];

    fn into_iter(self) -> Self::IntoIter {
        IterMut { slice_iter: self.buffers.iter_mut(), range: self.range.clone() }
    }
}

impl Buffer for SliceNoDrop {
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
        self.into_iter()
    }

    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
        self.into_iter()
    }

    fn len(&self) -> usize {
        self.range.len()
    }

    fn is_empty(&self) -> bool {
        self.range.len() == 0
    }

    fn empty() -> Self
    where
        Self: Sized,
    {
        Self { buffers: Vec::new(), range: 0..0 }
    }
}

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
        Some(SliceNoDrop { buffers: vec, range })
    }
}
