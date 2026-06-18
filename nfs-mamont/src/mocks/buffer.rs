use std::ops::Range;

use arbitrary::Unstructured;

use crate::Buffer;
#[cfg(feature = "arbitrary")]
pub const TEST_SIZE: usize = MAX_BLOCK_AMOUNT * MAX_BLOCK_SIZE;
#[cfg(feature = "arbitrary")]
pub const MAX_BLOCK_AMOUNT: usize = 64;
#[cfg(feature = "arbitrary")]
pub const MAX_BLOCK_SIZE: usize = 64;

struct MockBuffers {
    block_amounts: usize,
    block_size: usize,
    bufs: Vec<Box<[u8]>>,
    range: Range<usize>,
}

impl MockBuffers {
    fn new(block_amounts: usize, block_size: usize) -> Self {
        let mut bufs = Vec::with_capacity(block_amounts);
        for _ in 0..block_amounts {
            let buf = vec![0; block_size].into_boxed_slice();
            bufs.push(buf);
        }
        Self { block_amounts, block_size, bufs, range: 0..(block_size * block_amounts) }
    }

    fn iter(&self) -> impl Iterator<Item = &[u8]> {
        self.bufs.iter().map(|buf| buf.as_ref())
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut [u8]> {
        self.bufs.iter_mut().map(|buf| buf.as_mut())
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
        self.block_amounts * self.block_size
    }

    fn is_empty(&self) -> bool {
        self.bufs.is_empty() || self.bufs.iter().all(|buf| buf.is_empty())
    }

    fn empty() -> Self
    where
        Self: Sized,
    {
        Self::new(1, 0)
    }
}

#[cfg(feature = "arbitrary")]
impl PartialEq for MockBuffers {
    fn eq(&self, other: &Self) -> bool {
        if self.range == other.range {
            //yet we can't compare Slices
            return false;
        }
        for (left, right) in self.iter().zip(other.iter()) {
            if left != right {
                return false;
            }
        }
        true
    }
}

#[cfg(feature = "arbitrary")]
impl arbitrary::Arbitrary<'_> for MockBuffers {
    fn arbitrary(u: &mut Unstructured<'_>) -> arbitrary::Result<Self> {
        let block_amounts = u.int_in_range(1..=MAX_BLOCK_AMOUNT)?;
        let block_size = u.int_in_range(1..=MAX_BLOCK_SIZE)?;
        Ok(Self::new(block_amounts, block_size))
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
