use std::num::NonZeroUsize;

use arbitrary::{Arbitrary, Unstructured};

use nfs_mamont::{Allocator, Buffer};

pub mod parser_wrapper;
pub mod read_socket;
pub mod write_socket;

#[derive(Clone, Debug, Default)]
pub struct ZeroBuffers;

impl ZeroBuffers {
    pub fn new(_bufs: Vec<Box<[u8]>>, _len: usize) -> Self {
        Self { ..Self::default() }
    }

    pub fn empty() -> Self {
        Self::default()
    }
}

impl Buffer for ZeroBuffers {
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
        std::iter::empty::<&[u8]>()
    }

    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
        std::iter::empty::<&mut [u8]>()
    }

    fn len(&self) -> usize {
        0
    }

    fn is_empty(&self) -> bool {
        true
    }

    fn empty() -> Self
    where
        Self: Sized,
    {
        Self::default()
    }
}

impl PartialEq<[u8]> for ZeroBuffers {
    fn eq(&self, other: &[u8]) -> bool {
        other.is_empty()
    }
}

impl PartialEq<ZeroBuffers> for [u8] {
    fn eq(&self, _other: &ZeroBuffers) -> bool {
        self.is_empty()
    }
}

impl<'a> Arbitrary<'a> for ZeroBuffers {
    fn arbitrary(_u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(ZeroBuffers::default())
    }
}

pub struct ZeroAllocator;

impl ZeroAllocator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ZeroAllocator {
    fn default() -> Self {
        Self
    }
}

impl Allocator for ZeroAllocator {
    type Buffer = ZeroBuffers;

    async fn allocate(&self, _size: NonZeroUsize) -> Option<ZeroBuffers> {
        Some(Buffer::empty())
    }

    /// WARNING: this method return `NonZeroUsize::MAX` so that any allocation could be succeeded
    fn capacity(&self) -> NonZeroUsize {
        NonZeroUsize::MAX
    }
}
