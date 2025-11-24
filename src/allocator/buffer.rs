//! Defines heap allocated [`Buffer`] used to store raw-bytes.

use std::num::NonZeroUsize;

/// Pointer to the heap allocated [u8] slice.
pub struct Buffer(Box<[u8]>);

impl Buffer {
    /// Returns new heap allocated buffer, initialized with zeroed buffers of specified size.
    ///
    /// # Parameters:
    ///
    /// - `size` --- size of zero initialized buffer to allocate.
    pub fn new(size: NonZeroUsize) -> Self {
        Self(vec![0; size.get()].into_boxed_slice())
    }

    /// Returns contained number of bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl std::ops::Deref for Buffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl std::ops::DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut()
    }
}
