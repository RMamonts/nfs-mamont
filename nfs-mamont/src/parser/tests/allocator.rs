//! Test-only [`Allocator`] and [`Buffer`] backed by plain heap memory.

use std::num::NonZeroUsize;

use crate::allocator::{Allocator, Buffer};

/// Single-chunk [`Buffer`] that owns its memory.
#[derive(Debug, Default)]
pub struct MockBuffer(Box<[u8]>);

impl MockBuffer {
    /// Creates a zero-filled buffer of `len` bytes.
    pub fn zeroed(len: usize) -> Self {
        Self(vec![0; len].into_boxed_slice())
    }
}

impl Buffer for MockBuffer {
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
        std::iter::once(&self.0[..]).filter(|chunk| !chunk.is_empty())
    }

    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
        std::iter::once(&mut self.0[..]).filter(|chunk| !chunk.is_empty())
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn empty() -> Self {
        Self::default()
    }
}

impl PartialEq<[u8]> for MockBuffer {
    fn eq(&self, other: &[u8]) -> bool {
        *self.0 == *other
    }
}

impl PartialEq<MockBuffer> for [u8] {
    fn eq(&self, other: &MockBuffer) -> bool {
        *self == *other.0
    }
}

/// [`Allocator`] that hands out fresh [`MockBuffer`]s up to `max_size` bytes.
pub struct MockAllocator {
    max_size: usize,
}

impl MockAllocator {
    /// Creates an allocator that refuses requests larger than `max_size`.
    ///
    /// `max_size == 0` means "every allocation fails".
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }
}

impl Allocator for MockAllocator {
    type Buffer = MockBuffer;

    async fn allocate(&self, size: NonZeroUsize) -> Option<MockBuffer> {
        (size.get() <= self.max_size).then(|| MockBuffer::zeroed(size.get()))
    }

    /// `max_size == 0` means "every allocation fails"; it cannot be expressed as
    /// a [`NonZeroUsize`], so such a mock reports a capacity of one byte that
    /// [`Self::allocate`] still refuses.
    fn capacity(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.max_size).unwrap_or(NonZeroUsize::MIN)
    }
}
