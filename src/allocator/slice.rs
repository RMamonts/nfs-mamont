//! Defines [`Slice`] --- list of buffers bounded by custome byte range.

/// Represents bounded by custome range [`List`] of buffers.
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
    /// - `list` --- list of buffers.
    /// - `range` --- range to which slice will allow access.
    ///
    /// # Panics
    ///
    /// This function will panics if called if length range bound greater then length of `list`.
    pub fn new(
        buffers: Vec<Box<[u8]>>,
        range: std::ops::Range<usize>,
        sender: super::Sender<Box<[u8]>>,
    ) -> Self {
        let len = buffers.iter().map(|buffer| buffer.len()).sum();

        assert!(range.start <= len, "cannot index list as slice from start");
        assert!(range.end <= len, "cannot index list as slice to end");

        Self { buffers, range, sender }
    }

    pub fn iter_mut(&mut self) -> IterMut {
        self.into_iter()
    }

    pub fn iter(&self) -> Iter {
        self.into_iter()
    }

    /// Deallocates all buffers i.e. send them via specified in the [`Self::new`] `sender`.
    fn deallocate(&mut self) {
        for buffer in self.buffers.drain(..) {
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

            // make a progress
            self.range.start = self.range.start.saturating_sub(len);
            self.range.end = self.range.end.saturating_sub(len);

            if len > start {
                return Some(&result[self.range.start..end.min(len)]);
            }

            if self.range.end == 0 {
                return None;
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

            // make a progress
            self.range.start = self.range.start.saturating_sub(len);
            self.range.end = self.range.end.saturating_sub(len);

            if len > start {
                return Some(&mut result[self.range.start..end.min(len)]);
            }

            if self.range.end == 0 {
                return None;
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
