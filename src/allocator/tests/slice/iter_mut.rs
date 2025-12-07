//! Defines tests for [`crate::allocator::slice::Slice::iter_mut`] interface.

use tokio::sync::mpsc;

use crate::allocator::Receiver;
use crate::allocator::Slice;

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

// One buffer tests.

#[test]
fn zero_zero_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 0..0);
    let mut iter = slice.iter_mut();

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn one_one_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 1..1);
    let mut iter = slice.iter_mut();

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn end_end_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn zero_one_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 0..1);
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1].as_mut_slice()));
    assert!(iter.next().is_none());
}

#[test]
fn one_two_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 1..2);
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([2].as_mut_slice()));
    assert!(iter.next().is_none());
}

#[test]
fn last_byte_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], FIRST_BUFFER.len() - 1..FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([5].as_mut_slice()));
    assert!(iter.next().is_none());
}

#[test]
fn zero_half_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 0..FIRST_BUFFER.len() / 2);
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1, 2].as_mut_slice()));
    assert!(iter.next().is_none());
}

#[test]
fn half_end_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], FIRST_BUFFER.len() / 2..FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([3, 4, 5].as_mut_slice()));
    assert!(iter.next().is_none());
}

#[test]
fn zero_end_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 0..FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_mut_slice()));
    assert!(iter.next().is_none());
}

// Two buffers test, but range in first only.

#[test]
fn first_zero_zero_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..0);
    let mut iter = slice.iter_mut();

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn first_one_one_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 1..1);
    let mut iter = slice.iter_mut();

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn first_end_end_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn first_zero_one_two_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..1);
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1].as_mut_slice()));
    assert!(iter.next().is_none());
}

#[test]
fn first_one_two_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 1..2);
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([2].as_mut_slice()));
    assert!(iter.next().is_none());
}

#[test]
fn first_last_byte_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len() - 1..FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([5].as_mut_slice()));
    assert!(iter.next().is_none());
}

#[test]
fn first_zero_half_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len() / 2);
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1, 2].as_mut_slice()));
    assert!(iter.next().is_none());
}

#[test]
fn first_half_end_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len() / 2..FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([3, 4, 5].as_mut_slice()));
    assert!(iter.next().is_none());
}

#[test]
fn first_zero_end_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_mut_slice()));
    assert!(iter.next().is_none());
}

// Two buffers test, but range in second only.

#[test]
fn second_zero_zero_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn second_one_one_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], 1 + FIRST_BUFFER.len()..1 + FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn second_end_end_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len() + SECOND_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );
    let mut iter = slice.iter_mut();

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn second_zero_one_two_buffer() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len() + 1);
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([6].as_mut_slice()));
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn second_one_two_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len() + 1..FIRST_BUFFER.len() + 2);
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([7].as_mut_slice()));
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn second_last_byte_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len() + SECOND_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([8].as_mut_slice()));
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn second_zero_half_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len() / 2,
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([6].as_mut_slice()));
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn second_half_end_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len() + SECOND_BUFFER.len() / 2..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([7, 8].as_mut_slice()));
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn second_zero_end_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([6, 7, 8].as_mut_slice()));
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

// Two buffers, between them.

#[test]
fn last_from_first_first_from_second_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + 1);
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn all_from_first_first_from_second_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..1 + FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn last_from_first_all_from_second_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6, 7, 8].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn all_from_first_all_from_second_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len() + SECOND_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6, 7, 8].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

// Three buffers, between first and second.

#[test]
fn last_from_first_first_from_second_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + 1,
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn all_from_first_first_from_second_three_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER], 0..1 + FIRST_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn last_from_first_all_from_second_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6, 7, 8].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn all_from_first_all_from_second_three_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len() + SECOND_BUFFER.len());
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6, 7, 8].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

// Three buffers, between second and third.

#[test]
fn last_from_second_first_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() + SECOND_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([8].as_mut_slice()));
    assert_eq!(iter.next(), Some([9].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn all_from_second_first_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([6, 7, 8].as_mut_slice()));
    assert_eq!(iter.next(), Some([9].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn last_from_second_all_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() + SECOND_BUFFER.len() - 1
            ..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([8].as_mut_slice()));
    assert_eq!(iter.next(), Some([9, 10, 11].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn all_from_second_all_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([6, 7, 8].as_mut_slice()));
    assert_eq!(iter.next(), Some([9, 10, 11].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

// Three buffers, between first, second and third.

#[test]
fn last_from_first_all_from_second_first_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6, 7, 8].as_mut_slice()));
    assert_eq!(iter.next(), Some([9].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn all_from_first_all_from_second_first_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        0..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6, 7, 8].as_mut_slice()));
    assert_eq!(iter.next(), Some([9].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn last_from_first_all_from_second_all_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6, 7, 8].as_mut_slice()));
    assert_eq!(iter.next(), Some([9, 10, 11].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn all_from_first_all_from_second_all_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        0..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
    );
    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([1, 2, 3, 4, 5].as_mut_slice()));
    assert_eq!(iter.next(), Some([6, 7, 8].as_mut_slice()));
    assert_eq!(iter.next(), Some([9, 10, 11].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}
