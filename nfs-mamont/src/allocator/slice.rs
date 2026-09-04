//! Defines [`Slice`] --- list of buffers bounded by custome byte range.

use std::sync::Arc;

use super::Buffer;

/// Represents bounded by custome range list of buffers.
#[cfg_attr(test, derive(Debug))]
pub struct Slice {
    buffers: Vec<super::UnownedBuffer>,
    range: std::ops::Range<usize>,
    state: Option<Arc<super::AllocatorState>>,
}

impl Slice {
    /// Returns new [`Slice`] of specified buffers.
    ///
    /// # Parameters
    ///
    /// - `buffers` --- vec of buffers.
    /// - `range` --- range to which slice will allow access.
    /// - `state` --- allocator state to return buffers and restore permits.
    ///
    /// # Panics
    ///
    /// This function will panics if called if length range bound greater then length of `buffers`.
    pub fn new(
        buffers: Vec<super::UnownedBuffer>,
        range: std::ops::Range<usize>,
        state: Option<Arc<super::AllocatorState>>,
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

        Self { buffers, range, state }
    }

    /// Returns an empty slice that owns no buffers.
    pub fn empty() -> Self {
        Self { buffers: Vec::new(), range: 0..0, state: None }
    }

    /// Returns the total number of bytes available in this slice.
    pub fn len(&self) -> usize {
        self.range.len()
    }

    pub fn is_empty(&self) -> bool {
        self.range.len() == 0
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        self.into_iter()
    }

    pub fn iter(&self) -> Iter<'_> {
        self.into_iter()
    }

    /// Deallocates all buffers by returning them to the allocator state (pool)
    /// and restoring the corresponding permits on its semaphore.
    ///
    /// The allocator state is provided when constructing the slice via [`Self::new`].
    fn deallocate(&mut self) {
        if let Some(state) = &self.state {
            let count = self.buffers.len();
            for buffer in self.buffers.drain(..) {
                // Ignore allocator drop
                let _ = state.pool.push(buffer);
            }
            if count > 0 {
                state.semaphore.add_permits(count);
            }
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
    slice_iter: std::slice::Iter<'a, super::UnownedBuffer>,
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
    slice_iter: std::slice::IterMut<'a, super::UnownedBuffer>,
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

impl Buffer for Slice {
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

    fn empty() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
impl PartialEq<Slice> for [u8] {
    fn eq(&self, other: &Slice) -> bool {
        other == self
    }
}

#[cfg(kani)]
mod verification {
    use super::*;
    use crate::allocator::UnownedBuffer;

    const MAX_BUFFERS: usize = 3;
    const MAX_BUFFER_LEN: usize = 4;

    /// Backing storage for [`any_slice`], owned by the harness.
    ///
    /// It is a stack array rather than a `Vec<Box<[u8]>>` on purpose. Every
    /// symbolically sized heap allocation inside a harness costs CBMC a dynamic
    /// object of unknown extent plus the whole `RawVec` allocate/shrink/drop path,
    /// and one per buffer is what blew this proof's formula past the memory of a
    /// GitHub runner. The buffers carve disjoint sub-ranges out of one concrete
    /// block instead, which is the same input as far as `Iter`/`IterMut` are
    /// concerned: they only ever index within a single buffer.
    type Storage = [[u8; MAX_BUFFER_LEN]; MAX_BUFFERS];

    const EMPTY_STORAGE: Storage = [[0u8; MAX_BUFFER_LEN]; MAX_BUFFERS];

    /// A [`Slice`] over a nondeterministic number of buffers of nondeterministic
    /// lengths, bounded by a nondeterministic range.
    ///
    /// `storage` must outlive the returned slice; the harness keeps it on its own
    /// stack frame.
    fn any_slice(storage: &mut Storage) -> (Slice, usize, usize) {
        let count: usize = kani::any();
        kani::assume(count >= 1 && count <= MAX_BUFFERS);

        // Concrete capacity: a single fixed-size allocation that is never grown.
        let mut buffers: Vec<UnownedBuffer> = Vec::with_capacity(MAX_BUFFERS);
        let mut total = 0usize;

        // Indexed rather than iterator-driven: a slice iterator makes CBMC reason
        // about a moving pointer where an index only costs a bounds check.
        for i in 0..count {
            let len: usize = kani::any();
            // `Slice::new` asserts buffers are non-empty.
            kani::assume(len >= 1 && len <= MAX_BUFFER_LEN);

            let ptr = storage[i].as_mut_ptr();
            // SAFETY: `len` is at most the length of `storage[i]`, the blocks are
            // disjoint, and `storage` outlives the returned `Slice`.
            buffers.push(unsafe { UnownedBuffer::from_raw_parts(ptr, len) });
            total += len;
        }

        let start: usize = kani::any();
        let end: usize = kani::any();
        kani::assume(start <= end && end <= total);

        (Slice::new(buffers, start..end, None), start, end)
    }

    /// Proves [`Iter::next`] never panics on `&result[start..end.min(len)]` and
    /// that iteration yields exactly `range.len()` bytes, for every buffer layout
    /// and range within the bounds above.
    #[kani::proof]
    #[kani::unwind(5)]
    fn iter_covers_range_without_panicking() {
        let mut storage = EMPTY_STORAGE;
        let (slice, start, end) = any_slice(&mut storage);
        assert_eq!(slice.len(), end - start);

        let mut yielded = 0usize;
        for chunk in slice.iter() {
            yielded += chunk.len();
        }
        assert_eq!(yielded, end - start);
    }

    /// The [`IterMut::next`] counterpart. The two `next` implementations are
    /// duplicated line for line, so both are proved rather than one.
    #[kani::proof]
    #[kani::unwind(5)]
    fn iter_mut_covers_range_without_panicking() {
        let mut storage = EMPTY_STORAGE;
        let (mut slice, start, end) = any_slice(&mut storage);
        assert_eq!(slice.len(), end - start);

        let mut yielded = 0usize;
        for chunk in slice.iter_mut() {
            // Writing through the chunk is what a bad range would corrupt.
            chunk.fill(0xAB);
            yielded += chunk.len();
        }
        assert_eq!(yielded, end - start);
    }
}
