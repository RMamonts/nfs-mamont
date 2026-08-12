use super::{Iter, IterMut};
use crate::allocator::buffer::dropbuf::UnownedDroppableBuffer;
use crate::Buffer;

pub struct SliceNoDrop {
    buffers: Vec<UnownedDroppableBuffer>,
    range: std::ops::Range<usize>,
}

impl<'a> IntoIterator for &'a SliceNoDrop {
    type IntoIter = Iter<'a, UnownedDroppableBuffer>;
    type Item = &'a [u8];

    fn into_iter(self) -> Self::IntoIter {
        Iter { slice_iter: self.buffers.iter(), range: self.range.clone() }
    }
}

impl<'a> IntoIterator for &'a mut SliceNoDrop {
    type IntoIter = IterMut<'a, UnownedDroppableBuffer>;
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
