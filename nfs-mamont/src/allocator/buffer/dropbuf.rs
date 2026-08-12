use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use crate::allocator::dealloc::Deallocator;

impl super::RawBuffer for UnownedDroppableBuffer {
    fn len(&self) -> usize {
        Self::len(&self)
    }
}

#[derive(Clone)]
pub struct UnownedDroppableBuffer {
    ptr: *mut u8,
    len: usize,
    dealloc: Arc<dyn Deallocator<Self>>,
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
        dealloc: Arc<dyn Deallocator<Self>>,
    ) -> Self {
        Self { ptr, len, dealloc }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for UnownedDroppableBuffer {
    fn drop(&mut self) {
        self.dealloc.deallocate(self.clone());
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

impl AsRef<[u8]> for UnownedDroppableBuffer {
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}

impl AsMut<[u8]> for UnownedDroppableBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        self.deref_mut()
    }
}
