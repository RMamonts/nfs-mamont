//! Kani harnesses for the buffer pool and the [`Slice`] views over it.
//!
//! Sizes are tiny on purpose: the properties concern pointer arithmetic and
//! bookkeeping, which do not depend on how large the buffers are. Kani checks
//! every raw-pointer access in the harnesses for undefined behavior.

use std::num::NonZeroUsize;
use std::ops::Range;

use super::{Allocator, Impl, Slice, UnownedBuffer};

/// Upper bound on the size of one pooled buffer, in bytes.
const MAX_BUFFER_SIZE: usize = 4;
/// Upper bound on the number of pooled buffers.
const MAX_BUFFER_COUNT: usize = 3;

/// An allocator with arbitrary buffer size and count within the bounds above.
fn any_allocator() -> (Impl, usize, usize) {
    let size: usize = kani::any_where(|size| (1..=MAX_BUFFER_SIZE).contains(size));
    let count: usize = kani::any_where(|count| (1..=MAX_BUFFER_COUNT).contains(count));
    let allocator = Impl::new(NonZeroUsize::new(size).unwrap(), NonZeroUsize::new(count).unwrap());
    (allocator, size, count)
}

/// `Impl::new` carves its allocation into `count` disjoint buffers of `size`
/// bytes, each lying inside the allocation.
#[kani::proof]
#[kani::unwind(5)]
fn new_partitions_allocation_into_disjoint_buffers() {
    let (allocator, size, count) = any_allocator();
    let state = &allocator.state;

    let mut taken = [false; MAX_BUFFER_COUNT];
    let mut buffers = Vec::new();
    while let Some(buffer) = state.pool.pop() {
        assert_eq!(buffer.len(), size);
        // `offset_from` is only defined within one allocation, so Kani also
        // checks that the buffer points into the allocator's block.
        let offset = unsafe { buffer.as_ptr().offset_from(state.base_ptr) };
        let offset = usize::try_from(offset).unwrap();
        assert_eq!(offset % size, 0);
        let index = offset / size;
        assert!(index < count);
        assert!(!taken[index]);
        taken[index] = true;
        buffers.push(buffer);
    }
    assert_eq!(buffers.len(), count);

    for buffer in buffers {
        assert!(state.pool.push(buffer).is_ok());
    }
}

/// A request takes `ceil(request / size)` buffers and permits and yields a
/// slice of exactly `request` bytes; dropping the slice returns all of them.
/// A request above the capacity is refused without taking anything.
#[kani::proof]
#[kani::unwind(5)]
fn allocate_and_drop_conserve_buffers() {
    let (allocator, size, count) = any_allocator();
    let capacity = allocator.capacity().get();
    assert_eq!(capacity, size * count);
    let request: usize = kani::any_where(|request| (1..=capacity + 1).contains(request));

    let slice = kani::block_on(allocator.allocate(NonZeroUsize::new(request).unwrap()));
    let state = &allocator.state;
    let Some(slice) = slice else {
        assert!(request > capacity);
        assert_eq!(state.semaphore.available_permits(), count);
        assert_eq!(state.pool.len(), count);
        return;
    };

    let used = request.div_ceil(size);
    assert_eq!(slice.len(), request);
    assert_eq!(slice.iter().map(<[u8]>::len).sum::<usize>(), request);
    assert_eq!(state.semaphore.available_permits(), count - used);
    assert_eq!(state.pool.len(), count - used);

    drop(slice);
    assert_eq!(state.semaphore.available_permits(), count);
    assert_eq!(state.pool.len(), count);
}

/// Two live slices never share memory: writing through one leaves the other
/// intact.
#[kani::proof]
#[kani::unwind(5)]
fn live_slices_do_not_alias() {
    let (allocator, size, count) = any_allocator();
    kani::assume(count >= 2);
    let first_len: usize = kani::any_where(|len| (1..=size * (count - 1)).contains(len));
    let second_len: usize = kani::any_where(|len| (1..=size).contains(len));

    let mut first =
        kani::block_on(allocator.allocate(NonZeroUsize::new(first_len).unwrap())).unwrap();
    let mut second =
        kani::block_on(allocator.allocate(NonZeroUsize::new(second_len).unwrap())).unwrap();

    for chunk in first.iter_mut() {
        chunk.fill(0xAA);
    }
    for chunk in second.iter_mut() {
        chunk.fill(0x55);
    }
    assert!(first.iter().all(|chunk| chunk.iter().all(|byte| *byte == 0xAA)));
}

/// Stride between buffers in [`BufferLayout`]: buffer `i` starts at
/// `i * STRIDE` in the backing array.
const STRIDE: usize = MAX_BUFFER_SIZE;
/// Size of the backing array of a [`BufferLayout`].
const BACKING: usize = STRIDE * MAX_BUFFER_COUNT;

/// Up to [`MAX_BUFFER_COUNT`] buffers of arbitrary non-zero length laid out in
/// a backing array with gaps between them, so mixing up logical and physical
/// offsets changes the bytes a slice yields.
struct BufferLayout {
    count: usize,
    lens: [usize; MAX_BUFFER_COUNT],
}

impl BufferLayout {
    fn any() -> Self {
        let count: usize = kani::any_where(|count| *count <= MAX_BUFFER_COUNT);
        let lens = [(); MAX_BUFFER_COUNT]
            .map(|_| kani::any_where(|len: &usize| (1..=STRIDE).contains(len)));
        BufferLayout { count, lens }
    }

    /// Total number of bytes in all buffers.
    fn total(&self) -> usize {
        self.lens[..self.count].iter().sum()
    }

    /// Index in the backing array of logical byte `pos`.
    fn physical(&self, pos: usize) -> usize {
        let mut pos = pos;
        for (index, len) in self.lens[..self.count].iter().enumerate() {
            if pos < *len {
                return index * STRIDE + pos;
            }
            pos -= len;
        }
        unreachable!("logical position past the last buffer");
    }

    /// Logical position of the byte at `index` in the backing array, if any
    /// buffer covers it.
    fn logical(&self, index: usize) -> Option<usize> {
        let (buffer, offset) = (index / STRIDE, index % STRIDE);
        if buffer < self.count && offset < self.lens[buffer] {
            Some(self.lens[..buffer].iter().sum::<usize>() + offset)
        } else {
            None
        }
    }

    /// A slice over `backing` restricted to `range`, not tied to an allocator.
    fn slice(&self, backing: &mut [u8; BACKING], range: Range<usize>) -> Slice {
        let base = backing.as_mut_ptr();
        let buffers = (0..self.count)
            .map(|index| unsafe {
                UnownedBuffer::from_raw_parts(base.add(index * STRIDE), self.lens[index])
            })
            .collect();
        Slice::new(buffers, range, None)
    }
}

/// An arbitrary range within `0..total`.
fn any_range(total: usize) -> Range<usize> {
    let start: usize = kani::any_where(|start| *start <= total);
    let end: usize = kani::any_where(|end| (start..=total).contains(end));
    start..end
}

/// Iterating a slice yields non-empty chunks that concatenate to exactly the
/// bytes of its range.
#[kani::proof]
#[kani::unwind(6)]
fn iter_yields_exactly_the_range() {
    let mut backing: [u8; BACKING] = kani::any();
    let layout = BufferLayout::any();
    let range = any_range(layout.total());
    let slice = layout.slice(&mut backing, range.clone());
    assert_eq!(slice.len(), range.len());

    let mut pos = range.start;
    for chunk in slice.iter() {
        assert!(!chunk.is_empty());
        assert!(pos + chunk.len() <= range.end);
        for byte in chunk {
            assert_eq!(*byte, backing[layout.physical(pos)]);
            pos += 1;
        }
    }
    assert_eq!(pos, range.end);
}

/// Mutable iteration reaches exactly the bytes of the range: every byte inside
/// it can be written and no byte outside it is touched.
#[kani::proof]
#[kani::unwind(13)]
fn iter_mut_writes_exactly_the_range() {
    let mut backing: [u8; BACKING] = kani::any();
    let before = backing;
    let layout = BufferLayout::any();
    let range = any_range(layout.total());

    let mut slice = layout.slice(&mut backing, range.clone());
    let mut pos = range.start;
    for chunk in slice.iter_mut() {
        for byte in chunk {
            *byte = !before[layout.physical(pos)];
            pos += 1;
        }
    }
    assert_eq!(pos, range.end);
    drop(slice);

    for index in 0..BACKING {
        let inside = layout.logical(index).is_some_and(|pos| range.contains(&pos));
        assert_eq!(backing[index] != before[index], inside);
    }
}
