use std::alloc;
use std::alloc::Layout;
use std::ops::{Deref, DerefMut};

#[derive(Debug)]
#[cfg_attr(feature = "arbitrary", derive(Clone))]
pub struct UnownedBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

impl UnownedBuffer {
    /// Creates a new `UnownedBuffer` from raw parts.
    ///
    /// # Safety
    ///
    /// - `ptr` must be valid for reads and writes for `len` bytes.
    /// - `ptr` must be properly aligned for `u8`.
    /// - The caller must ensure that the memory is deallocated exactly once,
    ///   typically by the original allocator that owns the entire block.
    pub unsafe fn from_raw_parts(ptr: *mut u8, len: usize) -> Self {
        Self { ptr, len }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[cfg(feature = "arbitrary")]
    pub fn deallocate(self) {
        unsafe {
            let layout = match Layout::from_size_align(self.len, align_of::<u8>()) {
                Ok(layout) => layout,
                Err(_) => unreachable!("invalid layout"),
            };
            alloc::dealloc(self.ptr, layout)
        }
    }
}

unsafe impl Send for UnownedBuffer {}
unsafe impl Sync for UnownedBuffer {}

impl Deref for UnownedBuffer {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl DerefMut for UnownedBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}
