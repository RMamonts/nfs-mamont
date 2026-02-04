//! Defines [`Slice`] --- list of buffers bounded by custome byte range.
#[cfg(feature = "arbitrary")]
use arbitrary::{Arbitrary, Unstructured};
use tokio::sync::mpsc;

/// Represents bounded by custome range list of buffers.
#[derive(Debug)]
#[cfg_attr(feature = "arbitrary", derive(Clone))]
pub struct Slice {
    buffers: Vec<Box<[u8]>>,
    range: std::ops::Range<usize>,
    sender: super::Sender<Box<[u8]>>,
}

impl Slice {
    /// Returns new [`Slice`] of specified buffers.
    ///
    /// # Parameters
    ///
    /// - `buffers` --- vec of buffers.
    /// - `range` --- range to which slice will allow access.
    /// - `sender` --- sender to deallocated buffers in drop.
    ///
    /// # Panics
    ///
    /// This function will panics if called if length range bound greater then length of `buffers`.
    pub fn new(
        buffers: Vec<Box<[u8]>>,
        range: std::ops::Range<usize>,
        sender: super::Sender<Box<[u8]>>,
    ) -> Self {
        assert!(range.start <= range.end, "start should not be greater then end");

        let len = buffers
            .iter()
            .map(|buffer| {
                assert!(!buffer.is_empty());
                buffer.len()
            })
            .sum();

        assert!(range.start <= len, "cannot index list as slice from start");
        assert!(range.end <= len, "cannot index list as slice to end");

        Self { buffers, range, sender }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        self.into_iter()
    }

    pub fn iter(&self) -> Iter<'_> {
        self.into_iter()
    }

    /// Deallocates all buffers i.e. send them via specified in the [`Self::new`] `sender`.
    fn deallocate(&mut self) {
        for mut buffer in self.buffers.drain(..) {
            // No user data should exist after dealloc
            buffer.fill(0u8);
            // Ignore allocator drop
            let _ = self.sender.send(buffer);
        }
    }
}

impl Drop for Slice {
    fn drop(&mut self) {
        self.deallocate();
    }
}

/// Shared iterator over [`Slice`] buffers.
///
/// Return shared slices accordingly to [`Slice`] bounds.
pub struct Iter<'a> {
    slice_iter: std::slice::Iter<'a, Box<[u8]>>,
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

impl<'a> IntoIterator for &'a Slice {
    type IntoIter = Iter<'a>;
    type Item = &'a [u8];

    fn into_iter(self) -> Self::IntoIter {
        Iter { slice_iter: self.buffers.iter(), range: self.range.clone() }
    }
}

/// Unique iterator over [`Slice`] buffers.
///
/// Return mutable slices accordingly to [`Slice`] bounds.
pub struct IterMut<'a> {
    slice_iter: std::slice::IterMut<'a, Box<[u8]>>,
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

impl<'a> IntoIterator for &'a mut Slice {
    type IntoIter = IterMut<'a>;
    type Item = &'a mut [u8];

    fn into_iter(self) -> Self::IntoIter {
        IterMut { slice_iter: self.buffers.iter_mut(), range: self.range.clone() }
    }
}

impl PartialEq for Slice {
    fn eq(&self, other: &Self) -> bool {
        if self.range == other.range && self.buffers.len() == other.buffers.len() {
            for (buf1, buf2) in self.buffers.iter().zip(other.buffers.iter()) {
                if buf1.len() != buf2.len() {
                    break;
                }
            }
            return true;
        }
        false
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Slice {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let (sender, _) = mpsc::unbounded_channel();
        let length = u.int_in_range(1..=1000)?;
        let mut size = 0;
        let mut bufs = Vec::new();
        while size < length {
            let n = u.int_in_range(1..=(length - size))?;
            bufs.push(vec![8u8; n].into_boxed_slice());
            size += n;
        }
        Ok(Self::new(bufs, 0..length, sender))
    }
}
