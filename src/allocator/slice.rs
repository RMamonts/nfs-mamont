//! Defines [`Slice`] --- list of buffers bounded by custome byte range.

/// Represents bounded by custome range list of buffers.
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
        let len = buffers.iter().map(|buffer| buffer.len()).sum();

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

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use crate::allocator::Receiver;

    use super::Slice;

    const EMPTY_BUFFER: &[u8] = &[];
    const FIRST_BUFFER: &[u8] = &[1, 2, 3, 4, 5];
    const SECOND_BUFFER: &[u8] = &[6, 7, 8];
    const THIRD_BUFFER: &[u8] = &[9, 10, 11];

    fn make_slice<Buffers>(
        buffers: Buffers,
        range: std::ops::Range<usize>,
    ) -> (Slice, Receiver<Box<[u8]>>)
    where
        Buffers: IntoIterator<IntoIter: ExactSizeIterator<Item = &'static [u8]>>,
    {
        let buffers = buffers.into_iter();
        let (sender, receiver) = mpsc::unbounded_channel();

        let mut result = Vec::with_capacity(buffers.len());
        for slice in buffers {
            let mut buf = Vec::with_capacity(slice.len());
            buf.extend_from_slice(slice);

            result.push(buf.into_boxed_slice())
        }

        let slice = Slice::new(result, range, sender);

        (slice, receiver)
    }

    // One buffer tests

    #[test]
    fn zero_zero_empty_buffer() {
        let (slice, _) = make_slice([EMPTY_BUFFER], 0..0);

        let mut iter = slice.iter();

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn zero_zero_one_buffer() {
        let (slice, _) = make_slice([FIRST_BUFFER], 0..0);
        let mut iter = slice.iter();

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn one_one_one_buffer() {
        let (slice, _) = make_slice([FIRST_BUFFER], 1..1);
        let mut iter = slice.iter();

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn end_end_one_buffer() {
        let (slice, _) = make_slice([FIRST_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len());
        let mut iter = slice.iter();

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn zero_one_one_buffer() {
        let (slice, _) = make_slice([FIRST_BUFFER], 0..1);
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1].as_slice()));
        assert!(iter.next().is_none());
    }

    #[test]
    fn one_two_one_buffer() {
        let (slice, _) = make_slice([FIRST_BUFFER], 1..2);
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([2].as_slice()));
        assert!(iter.next().is_none());
    }

    #[test]
    fn last_byte_one_buffer() {
        let (slice, _) = make_slice([FIRST_BUFFER], FIRST_BUFFER.len() - 1..FIRST_BUFFER.len());
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([5].as_slice()));
        assert!(iter.next().is_none());
    }

    #[test]
    fn zero_half_one_buffer() {
        let (slice, _) = make_slice([FIRST_BUFFER], 0..FIRST_BUFFER.len() / 2);
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1, 2].as_slice()));
        assert!(iter.next().is_none());
    }

    #[test]
    fn half_end_one_buffer() {
        let (slice, _) = make_slice([FIRST_BUFFER], FIRST_BUFFER.len() / 2..FIRST_BUFFER.len());
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([3, 4, 5].as_slice()));
        assert!(iter.next().is_none());
    }

    #[test]
    fn zero_end_one_buffer() {
        let (slice, _) = make_slice([FIRST_BUFFER], 0..FIRST_BUFFER.len());
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_slice()));
        assert!(iter.next().is_none());
    }

    // two buffers test, but range in first only

    #[test]
    fn zero_zero_two_empty_buffers() {
        let (slice, _) = make_slice([EMPTY_BUFFER, EMPTY_BUFFER], 0..0);

        let mut iter = slice.iter();

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn first_zero_zero_two_buffers() {
        let (slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..0);
        let mut iter = slice.iter();

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn first_one_one_two_buffers() {
        let (slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 1..1);
        let mut iter = slice.iter();

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn first_end_end_two_buffers() {
        let (slice, _) =
            make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len());
        let mut iter = slice.iter();

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn first_zero_one_two_buffer() {
        let (slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..1);
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1].as_slice()));
        assert!(iter.next().is_none());
    }

    #[test]
    fn first_one_two_two_buffers() {
        let (slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 1..2);
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([2].as_slice()));
        assert!(iter.next().is_none());
    }

    #[test]
    fn first_last_byte_two_buffers() {
        let (slice, _) =
            make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len() - 1..FIRST_BUFFER.len());
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([5].as_slice()));
        assert!(iter.next().is_none());
    }

    #[test]
    fn first_zero_half_two_buffers() {
        let (slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len() / 2);
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1, 2].as_slice()));
        assert!(iter.next().is_none());
    }

    #[test]
    fn first_half_end_two_buffers() {
        let (slice, _) =
            make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len() / 2..FIRST_BUFFER.len());
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([3, 4, 5].as_slice()));
        assert!(iter.next().is_none());
    }

    #[test]
    fn first_zero_end_two_buffers() {
        let (slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len());
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_slice()));
        assert!(iter.next().is_none());
    }

    // two buffers test, but range in second only

    #[test]
    fn second_zero_zero_two_buffers() {
        let (slice, _) =
            make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len());
        let mut iter = slice.iter();

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn second_one_one_two_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER],
            1 + FIRST_BUFFER.len()..1 + FIRST_BUFFER.len(),
        );
        let mut iter = slice.iter();

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn second_end_end_two_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER],
            FIRST_BUFFER.len() + SECOND_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
        );
        let mut iter = slice.iter();

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn second_zero_one_two_buffer() {
        let (slice, _) =
            make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len() + 1);
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([6].as_slice()));
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn second_one_two_two_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER],
            FIRST_BUFFER.len() + 1..FIRST_BUFFER.len() + 2,
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([7].as_slice()));
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn second_last_byte_two_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER],
            FIRST_BUFFER.len() + SECOND_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([8].as_slice()));
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn second_zero_half_two_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER],
            FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len() / 2,
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([6].as_slice()));
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn second_half_end_two_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER],
            FIRST_BUFFER.len() + SECOND_BUFFER.len() / 2..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([7, 8].as_slice()));
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn second_zero_end_two_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER],
            FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([6, 7, 8].as_slice()));
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    // two buffers, between them

    #[test]
    fn last_from_first_first_from_second_two_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER],
            FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + 1,
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([5].as_slice()));
        assert_eq!(iter.next(), Some([6].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn all_from_first_first_from_second_two_buffers() {
        let (slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..1 + FIRST_BUFFER.len());
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_slice()));
        assert_eq!(iter.next(), Some([6].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn last_from_first_all_from_second_two_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER],
            FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([5].as_slice()));
        assert_eq!(iter.next(), Some([6, 7, 8].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn all_from_first_all_from_second_two_buffers() {
        let (slice, _) =
            make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len() + SECOND_BUFFER.len());
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_slice()));
        assert_eq!(iter.next(), Some([6, 7, 8].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    // three buffers, between first and second

    #[test]
    fn last_from_first_first_from_second_three_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
            FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + 1,
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([5].as_slice()));
        assert_eq!(iter.next(), Some([6].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn all_from_first_first_from_second_three_buffers() {
        let (slice, _) =
            make_slice([FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER], 0..1 + FIRST_BUFFER.len());
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_slice()));
        assert_eq!(iter.next(), Some([6].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn last_from_first_all_from_second_three_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
            FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([5].as_slice()));
        assert_eq!(iter.next(), Some([6, 7, 8].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn all_from_first_all_from_second_three_buffers() {
        let (slice, _) =
            make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len() + SECOND_BUFFER.len());
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_slice()));
        assert_eq!(iter.next(), Some([6, 7, 8].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    // three buffers, between second and third

    #[test]
    fn last_from_second_first_from_third_three_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
            FIRST_BUFFER.len() + SECOND_BUFFER.len() - 1
                ..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([8].as_slice()));
        assert_eq!(iter.next(), Some([9].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn all_from_second_first_from_third_three_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
            FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([6, 7, 8].as_slice()));
        assert_eq!(iter.next(), Some([9].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn last_from_second_all_from_third_three_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
            FIRST_BUFFER.len() + SECOND_BUFFER.len() - 1
                ..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([8].as_slice()));
        assert_eq!(iter.next(), Some([9, 10, 11].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn all_from_second_all_from_third_three_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
            FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([6, 7, 8].as_slice()));
        assert_eq!(iter.next(), Some([9, 10, 11].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    // three buffers, between first, second and third

    #[test]
    fn last_from_first_all_from_second_first_from_third_three_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
            FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([5].as_slice()));
        assert_eq!(iter.next(), Some([6, 7, 8].as_slice()));
        assert_eq!(iter.next(), Some([9].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn all_from_first_all_from_second_first_from_third_three_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
            0..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_slice()));
        assert_eq!(iter.next(), Some([6, 7, 8].as_slice()));
        assert_eq!(iter.next(), Some([9].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn last_from_first_all_from_second_all_from_third_three_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
            FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([5].as_slice()));
        assert_eq!(iter.next(), Some([6, 7, 8].as_slice()));
        assert_eq!(iter.next(), Some([9, 10, 11].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }

    #[test]
    fn all_from_first_all_from_second_all_from_third_three_buffers() {
        let (slice, _) = make_slice(
            [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
            0..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
        );
        let mut iter = slice.iter();

        assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_slice()));
        assert_eq!(iter.next(), Some([6, 7, 8].as_slice()));
        assert_eq!(iter.next(), Some([9, 10, 11].as_slice()));

        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }
}
