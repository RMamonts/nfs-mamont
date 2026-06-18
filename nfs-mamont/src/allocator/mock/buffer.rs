use std::ops::Range;

#[cfg(feature = "arbitrary")]
use arbitrary::Unstructured;

use crate::Buffer;
#[cfg(feature = "arbitrary")]
pub const TEST_SIZE: usize = MAX_BLOCK_AMOUNT * MAX_BLOCK_SIZE;
#[cfg(feature = "arbitrary")]
pub const MAX_BLOCK_AMOUNT: usize = 64;
#[cfg(feature = "arbitrary")]
pub const MAX_BLOCK_SIZE: usize = 64;

#[derive(Clone, Debug)]
pub struct MockBuffers {
    bufs: Vec<Box<[u8]>>,
    range: Range<usize>,
}

impl MockBuffers {
    pub fn new(vec: Vec<Box<[u8]>>, len: usize) -> Self {
        Self { bufs: vec, range: 0..len }
    }
    fn iter(&self) -> impl Iterator<Item = &[u8]> + '_ {
        BufferIter { bufs: self.bufs.iter(), range: self.range.clone() }
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + '_ {
        BufferIterMut { bufs: self.bufs.iter_mut(), range: self.range.clone() }
    }
}

/// Iterator over MockBuffers respecting the range bounds
struct BufferIter<'a> {
    bufs: std::slice::Iter<'a, Box<[u8]>>,
    range: Range<usize>,
}

impl<'a> Iterator for BufferIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let result = self.bufs.next()?;
            let len = result.len();
            let start = self.range.start;
            let end = self.range.end;

            if start == end {
                return None;
            }

            self.range.start = self.range.start.saturating_sub(len);
            self.range.end = self.range.end.saturating_sub(len);

            if len > start {
                return Some(&result[start..end.min(len)]);
            }
        }
    }
}

/// Mutable iterator over MockBuffers respecting the range bounds
struct BufferIterMut<'a> {
    bufs: std::slice::IterMut<'a, Box<[u8]>>,
    range: Range<usize>,
}

impl<'a> Iterator for BufferIterMut<'a> {
    type Item = &'a mut [u8];

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let result = self.bufs.next()?;
            let len = result.len();
            let start = self.range.start;
            let end = self.range.end;

            if start == end {
                return None;
            }

            self.range.start = self.range.start.saturating_sub(len);
            self.range.end = self.range.end.saturating_sub(len);

            if len > start {
                return Some(&mut result[start..end.min(len)]);
            }
        }
    }
}

impl Buffer for MockBuffers {
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
        self.iter()
    }

    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
        self.iter_mut()
    }

    fn len(&self) -> usize {
        self.range.end - self.range.start
    }

    fn is_empty(&self) -> bool {
        self.bufs.is_empty() || self.bufs.iter().all(|buf| buf.is_empty())
    }

    fn empty() -> Self
    where
        Self: Sized,
    {
        Self::new(vec![], 0)
    }
}

#[cfg(feature = "arbitrary")]
impl arbitrary::Arbitrary<'_> for MockBuffers {
    fn arbitrary(u: &mut Unstructured<'_>) -> arbitrary::Result<Self> {
        let block_amounts = u.int_in_range(1..=MAX_BLOCK_AMOUNT)?;
        let block_size = u.int_in_range(1..=MAX_BLOCK_SIZE)?;
        let vec = (0..block_amounts)
            .map(|_| {
                let buf = vec![0; block_size].into_boxed_slice();
                Ok(buf)
            })
            .collect::<arbitrary::Result<Vec<Box<[u8]>>>>()?;
        Ok(Self::new(vec, block_size * block_amounts))
    }
}

#[cfg(test)]
impl PartialEq<[u8]> for MockBuffers {
    fn eq(&self, other: &[u8]) -> bool {
        if self.range.len() == 1 && other.is_empty() {
            return true;
        }

        if self.range.len() != other.len() {
            return false;
        }

        let mut self_iter = self.iter();
        let mut block_self = self_iter.next();

        let mut other = other;

        loop {
            match block_self {
                None => return other.is_empty(),
                Some(mut cur_self) => {
                    loop {
                        let take = cur_self.len().min(other.len());

                        if cur_self[..take] != other[..take] {
                            return false;
                        }

                        cur_self = &cur_self[take..];
                        other = &other[take..];

                        if cur_self.is_empty() || other.is_empty() {
                            break;
                        }
                    }

                    block_self =
                        if cur_self.is_empty() { self_iter.next() } else { Some(cur_self) };
                }
            }
        }
    }
}
#[cfg(test)]
impl PartialEq<MockBuffers> for [u8] {
    fn eq(&self, other: &MockBuffers) -> bool {
        other == self
    }
}
