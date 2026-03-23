//! Defines [`Slice`] --- list of buffers bounded by custome byte range.

use tokio::sync::mpsc;

/// Represents bounded by custome range list of buffers.
#[cfg_attr(test, derive(Debug))]
pub struct Slice {
    pub(super) buffers: Vec<Box<[u8]>>,
    pub(super) range: std::ops::Range<usize>,
    pub(super) sender: super::Sender<Box<[u8]>>,
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

    // /// Returns an empty slice that owns no buffers.
    pub fn empty() -> Self {
        let (sender, _receiver) = mpsc::unbounded_channel::<Box<[u8]>>();
        Self { buffers: Vec::new(), range: 0..0, sender }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        self.into_iter()
    }

    pub fn iter(&self) -> Iter<'_> {
        self.into_iter()
    }

    /// Deallocates all buffers i.e. send them via specified in the [`Self::new`] `sender`.
    pub(super) fn deallocate(&mut self) {
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
    pub(super) slice_iter: std::slice::Iter<'a, Box<[u8]>>,
    pub(super) range: std::ops::Range<usize>,
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
    type Item = &'a [u8];
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter { slice_iter: self.buffers.iter(), range: self.range.clone() }
    }
}

/// Unique iterator over [`Slice`] buffers.
///
/// Return mutable slices accordingly to [`Slice`] bounds.
pub struct IterMut<'a> {
    pub(super) slice_iter: std::slice::IterMut<'a, Box<[u8]>>,
    pub(super) range: std::ops::Range<usize>,
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

#[cfg(test)]
impl PartialEq<[u8]> for Slice {
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
impl PartialEq<Slice> for [u8] {
    fn eq(&self, other: &Slice) -> bool {
        other == self
    }
}
