use std::num::NonZeroUsize;

/// Pointer to the heap allocated [u8] slice.
pub struct Buffer(Vec<u8>);

impl Buffer {
    /// Allocates zeroed buffer with specified size.
    ///
    /// # Parameters:
    /// - `size` --- size of buffer to allocate.
    pub fn new(size: NonZeroUsize) -> Self {
        Self(vec![0; size.get()])
    }

    /// Returns mutable slice to inner buffer.
    pub fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }

    /// Returns length of the buffer.
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
