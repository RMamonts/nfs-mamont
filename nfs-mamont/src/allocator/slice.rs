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

    /// Byte written through [`IterMut`]; distinct from the zeroed storage so an
    /// out-of-range write is visible.
    const FILL: u8 = 0xAB;

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

    /// A nondeterministic [`Slice`] together with the shape it was built from.
    struct AnySlice {
        slice: Slice,
        /// Number of buffers; entries of `lens` past it are unused.
        count: usize,
        /// Length of each buffer, i.e. how much of `storage[i]` it covers.
        lens: [usize; MAX_BUFFERS],
        /// The range the slice was built with.
        start: usize,
        end: usize,
    }

    /// A [`Slice`] over a nondeterministic number of buffers of nondeterministic
    /// lengths, bounded by a nondeterministic range.
    ///
    /// `storage` must outlive the returned slice; the harness keeps it on its own
    /// stack frame.
    fn any_slice(storage: &mut Storage) -> AnySlice {
        let count: usize = kani::any();
        kani::assume(count >= 1 && count <= MAX_BUFFERS);

        // Concrete capacity: a single fixed-size allocation that is never grown.
        let mut buffers: Vec<UnownedBuffer> = Vec::with_capacity(MAX_BUFFERS);
        let mut lens = [0usize; MAX_BUFFERS];
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
            lens[i] = len;
            total += len;
        }

        let start: usize = kani::any();
        let end: usize = kani::any();
        kani::assume(start <= end && end <= total);

        AnySlice { slice: Slice::new(buffers, start..end, None), count, lens, start, end }
    }

    /// Proves [`Iter::next`] never panics on `&result[start..end.min(len)]` and
    /// that iteration yields exactly `range.len()` bytes, for every buffer layout
    /// and range within the bounds above.
    ///
    /// The chunks are also non-empty: callers such as
    /// [`crate::parser::parser_struct::read_in_slice_async`] subtract a chunk
    /// length from a remaining counter and would stall on an empty one.
    #[kani::proof]
    #[kani::unwind(5)]
    fn iter_covers_range_without_panicking() {
        let mut storage = EMPTY_STORAGE;
        let any = any_slice(&mut storage);
        assert_eq!(any.slice.len(), any.end - any.start);

        let mut yielded = 0usize;
        let mut chunks = 0usize;
        for chunk in any.slice.iter() {
            assert!(!chunk.is_empty());
            yielded += chunk.len();
            chunks += 1;
        }
        assert_eq!(yielded, any.end - any.start);
        assert!(chunks <= any.count);
    }

    /// The [`IterMut::next`] counterpart. The two `next` implementations are
    /// duplicated line for line, so both are proved rather than one.
    #[kani::proof]
    #[kani::unwind(5)]
    fn iter_mut_covers_range_without_panicking() {
        let mut storage = EMPTY_STORAGE;
        let mut any = any_slice(&mut storage);
        assert_eq!(any.slice.len(), any.end - any.start);

        let mut yielded = 0usize;
        let mut chunks = 0usize;
        for chunk in any.slice.iter_mut() {
            assert!(!chunk.is_empty());
            // Writing through the chunk is what a bad range would corrupt.
            chunk.fill(FILL);
            yielded += chunk.len();
            chunks += 1;
        }
        assert_eq!(yielded, any.end - any.start);
        assert!(chunks <= any.count);
    }

    /// Writing through [`IterMut`] touches the bytes the range names and nothing
    /// else.
    ///
    /// A slice hands out memory the allocator pool also lends to other requests,
    /// so a chunk that reached one byte past its range would corrupt another
    /// connection's payload --- silently, and only for particular buffer layouts.
    /// The byte counter in [`Self::iter_mut_covers_range_without_panicking`]
    /// cannot see that: a chunk shifted by one still has the right length.
    #[kani::proof]
    #[kani::unwind(5)]
    fn iter_mut_writes_stay_inside_range() {
        let mut storage = EMPTY_STORAGE;
        let mut any = any_slice(&mut storage);

        for chunk in any.slice.iter_mut() {
            chunk.fill(FILL);
        }
        // Ends the slice's borrow of `storage` before the bytes are read back.
        drop(any.slice);

        // Every byte of the storage, not just the ones the buffers cover: a write
        // that ran one byte past a buffer, or into a block no buffer was built
        // from, lands here too. `lens[i]` is zero for the blocks past `count`.
        let mut offset = 0usize;
        for i in 0..MAX_BUFFERS {
            for j in 0..MAX_BUFFER_LEN {
                let in_range = i < any.count
                    && j < any.lens[i]
                    && offset + j >= any.start
                    && offset + j < any.end;
                assert_eq!(storage[i][j] == FILL, in_range);
            }
            offset += any.lens[i];
        }
    }
}
