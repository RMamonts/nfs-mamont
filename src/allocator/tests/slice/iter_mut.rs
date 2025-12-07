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

fn check_iter_is_empty<'a>(iter: &'a mut impl Iterator<Item = &'a mut [u8]>) {
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

fn check_slice_is_empty(slice: &mut Slice) {
    check_iter_is_empty(&mut slice.iter_mut());
}

fn check_slice_content<Content>(slice: &mut Slice, content: Content)
where
    Content: IntoIterator<IntoIter: Iterator<Item: AsRef<[u8]>>>,
{
    let mut iter_under_test = slice.iter_mut();

    for expected_slice in content.into_iter() {
        let actual_slice = iter_under_test.next().unwrap();

        assert_eq!(actual_slice, expected_slice.as_ref());
    }

    check_iter_is_empty(&mut iter_under_test);
}

// One buffer tests.

#[test]
fn zero_zero_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 0..0);

    check_slice_is_empty(&mut slice);
    check_slice_is_empty(&mut slice);
}

#[test]
fn one_one_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 1..1);

    check_slice_is_empty(&mut slice);
    check_slice_is_empty(&mut slice);
}

#[test]
fn end_end_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len());

    check_slice_is_empty(&mut slice);
    check_slice_is_empty(&mut slice);
}

#[test]
fn zero_one_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 0..1);

    check_slice_content(&mut slice, [&[1]]);
    check_slice_content(&mut slice, [&[1]]);
}

#[test]
fn one_two_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 1..2);

    check_slice_content(&mut slice, [&[2]]);
    check_slice_content(&mut slice, [&[2]]);
}

#[test]
fn last_byte_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], FIRST_BUFFER.len() - 1..FIRST_BUFFER.len());

    check_slice_content(&mut slice, [&[5]]);
    check_slice_content(&mut slice, [&[5]]);
}

#[test]
fn zero_half_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 0..FIRST_BUFFER.len() / 2);

    check_slice_content(&mut slice, [&[1, 2]]);
    check_slice_content(&mut slice, [&[1, 2]]);
}

#[test]
fn half_end_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], FIRST_BUFFER.len() / 2..FIRST_BUFFER.len());

    check_slice_content(&mut slice, [&[3, 4, 5]]);
    check_slice_content(&mut slice, [&[3, 4, 5]]);
}

#[test]
fn zero_end_one_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 0..FIRST_BUFFER.len());

    check_slice_content(&mut slice, [&[1, 2, 3, 4, 5]]);
    check_slice_content(&mut slice, [&[1, 2, 3, 4, 5]]);
}

// Two buffers test, but range in first only.

#[test]
fn first_zero_zero_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..0);

    check_slice_is_empty(&mut slice);
    check_slice_is_empty(&mut slice);
}

#[test]
fn first_one_one_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 1..1);

    check_slice_is_empty(&mut slice);
    check_slice_is_empty(&mut slice);
}

#[test]
fn first_end_end_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len());

    check_slice_is_empty(&mut slice);
    check_slice_is_empty(&mut slice);
}

#[test]
fn first_zero_one_two_buffer() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..1);

    check_slice_content(&mut slice, [&[1]]);
    check_slice_content(&mut slice, [&[1]]);
}

#[test]
fn first_one_two_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 1..2);

    check_slice_content(&mut slice, [&[2]]);
    check_slice_content(&mut slice, [&[2]]);
}

#[test]
fn first_last_byte_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len() - 1..FIRST_BUFFER.len());

    check_slice_content(&mut slice, [&[5]]);
    check_slice_content(&mut slice, [&[5]]);
}

#[test]
fn first_zero_half_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len() / 2);

    check_slice_content(&mut slice, [&[1, 2]]);
    check_slice_content(&mut slice, [&[1, 2]]);
}

#[test]
fn first_half_end_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len() / 2..FIRST_BUFFER.len());

    check_slice_content(&mut slice, [&[3, 4, 5]]);
    check_slice_content(&mut slice, [&[3, 4, 5]]);
}

#[test]
fn first_zero_end_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len());

    check_slice_content(&mut slice, [&[1, 2, 3, 4, 5]]);
    check_slice_content(&mut slice, [&[1, 2, 3, 4, 5]]);
}

// Two buffers test, but range in second only.

#[test]
fn second_zero_zero_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len());

    check_slice_is_empty(&mut slice);
    check_slice_is_empty(&mut slice);
}

#[test]
fn second_one_one_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], 1 + FIRST_BUFFER.len()..1 + FIRST_BUFFER.len());

    check_slice_is_empty(&mut slice);
    check_slice_is_empty(&mut slice);
}

#[test]
fn second_end_end_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len() + SECOND_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );

    check_slice_is_empty(&mut slice);
    check_slice_is_empty(&mut slice);
}

#[test]
fn second_zero_one_two_buffer() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len()..FIRST_BUFFER.len() + 1);

    check_slice_content(&mut slice, [&[6]]);
    check_slice_content(&mut slice, [&[6]]);
}

#[test]

fn second_one_two_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len() + 1..FIRST_BUFFER.len() + 2);

    check_slice_content(&mut slice, [&[7]]);
    check_slice_content(&mut slice, [&[7]]);
}

#[test]
fn second_last_byte_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len() + SECOND_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );

    check_slice_content(&mut slice, [&[8]]);
    check_slice_content(&mut slice, [&[8]]);
}

#[test]
fn second_zero_half_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len() / 2,
    );

    check_slice_content(&mut slice, [&[6]]);
    check_slice_content(&mut slice, [&[6]]);
}

#[test]
fn second_half_end_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len() + SECOND_BUFFER.len() / 2..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );

    check_slice_content(&mut slice, [&[7, 8]]);
    check_slice_content(&mut slice, [&[7, 8]]);
}

#[test]
fn second_zero_end_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );

    check_slice_content(&mut slice, [&[6, 7, 8]]);
    check_slice_content(&mut slice, [&[6, 7, 8]]);
}

// Two buffers, between them.

#[test]
fn last_from_first_first_from_second_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + 1);

    check_slice_content(&mut slice, [&[5], &[6]]);
    check_slice_content(&mut slice, [&[5], &[6]]);
}

#[test]
fn all_from_first_first_from_second_two_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..1 + FIRST_BUFFER.len());

    check_slice_content(&mut slice, [[1, 2, 3, 4, 5].as_slice(), [6].as_slice()]);
    check_slice_content(&mut slice, [[1, 2, 3, 4, 5].as_slice(), [6].as_slice()]);
}

#[test]
fn last_from_first_all_from_second_two_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER],
        FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );

    check_slice_content(&mut slice, [[5].as_slice(), [6, 7, 8].as_slice()]);
    check_slice_content(&mut slice, [[5].as_slice(), [6, 7, 8].as_slice()]);
}

#[test]
fn all_from_first_all_from_second_two_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len() + SECOND_BUFFER.len());

    check_slice_content(&mut slice, [[1, 2, 3, 4, 5].as_slice(), [6, 7, 8].as_slice()]);
    check_slice_content(&mut slice, [[1, 2, 3, 4, 5].as_slice(), [6, 7, 8].as_slice()]);
}

// Three buffers, between first and second.

#[test]
fn last_from_first_first_from_second_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + 1,
    );

    check_slice_content(&mut slice, [[5].as_slice(), [6].as_slice()]);
    check_slice_content(&mut slice, [[5].as_slice(), [6].as_slice()]);
}

#[test]
fn all_from_first_first_from_second_three_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER], 0..1 + FIRST_BUFFER.len());

    check_slice_content(&mut slice, [[1, 2, 3, 4, 5].as_slice(), [6].as_slice()]);
    check_slice_content(&mut slice, [[1, 2, 3, 4, 5].as_slice(), [6].as_slice()]);
}

#[test]
fn last_from_first_all_from_second_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len(),
    );

    check_slice_content(&mut slice, [[5].as_slice(), [6, 7, 8].as_slice()]);
    check_slice_content(&mut slice, [[5].as_slice(), [6, 7, 8].as_slice()]);
}

#[test]
fn all_from_first_all_from_second_three_buffers() {
    let (mut slice, _) =
        make_slice([FIRST_BUFFER, SECOND_BUFFER], 0..FIRST_BUFFER.len() + SECOND_BUFFER.len());

    check_slice_content(&mut slice, [[1, 2, 3, 4, 5].as_slice(), [6, 7, 8].as_slice()]);
    check_slice_content(&mut slice, [[1, 2, 3, 4, 5].as_slice(), [6, 7, 8].as_slice()]);
}

// Three buffers, between second and third.

#[test]
fn last_from_second_first_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() + SECOND_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
    );

    check_slice_content(&mut slice, [[8].as_slice(), [9].as_slice()]);
    check_slice_content(&mut slice, [[8].as_slice(), [9].as_slice()]);
}

#[test]
fn all_from_second_first_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
    );

    check_slice_content(&mut slice, [[6, 7, 8].as_slice(), [9].as_slice()]);
    check_slice_content(&mut slice, [[6, 7, 8].as_slice(), [9].as_slice()]);
}

#[test]
fn last_from_second_all_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() + SECOND_BUFFER.len() - 1
            ..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
    );

    check_slice_content(&mut slice, [[8].as_slice(), [9, 10, 11].as_slice()]);
    check_slice_content(&mut slice, [[8].as_slice(), [9, 10, 11].as_slice()]);
}

#[test]
fn all_from_second_all_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len()..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
    );

    check_slice_content(&mut slice, [[6, 7, 8].as_slice(), [9, 10, 11].as_slice()]);
    check_slice_content(&mut slice, [[6, 7, 8].as_slice(), [9, 10, 11].as_slice()]);
}

// Three buffers, between first, second and third.

#[test]
fn last_from_first_all_from_second_first_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
    );

    check_slice_content(&mut slice, [[5].as_slice(), [6, 7, 8].as_slice(), [9].as_slice()]);
    check_slice_content(&mut slice, [[5].as_slice(), [6, 7, 8].as_slice(), [9].as_slice()]);
}

#[test]
fn all_from_first_all_from_second_first_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        0..FIRST_BUFFER.len() + SECOND_BUFFER.len() + 1,
    );

    check_slice_content(
        &mut slice,
        [[1, 2, 3, 4, 5].as_slice(), [6, 7, 8].as_slice(), [9].as_slice()],
    );
    check_slice_content(
        &mut slice,
        [[1, 2, 3, 4, 5].as_slice(), [6, 7, 8].as_slice(), [9].as_slice()],
    );
}

#[test]
fn last_from_first_all_from_second_all_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        FIRST_BUFFER.len() - 1..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
    );

    check_slice_content(&mut slice, [[5].as_slice(), [6, 7, 8].as_slice(), [9, 10, 11].as_slice()]);
    check_slice_content(&mut slice, [[5].as_slice(), [6, 7, 8].as_slice(), [9, 10, 11].as_slice()]);
}

#[test]
fn all_from_first_all_from_second_all_from_third_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        0..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
    );

    check_slice_content(
        &mut slice,
        [[1, 2, 3, 4, 5].as_slice(), [6, 7, 8].as_slice(), [9, 10, 11].as_slice()],
    );
    check_slice_content(
        &mut slice,
        [[1, 2, 3, 4, 5].as_slice(), [6, 7, 8].as_slice(), [9, 10, 11].as_slice()],
    );
}

// Checks that we can observe what writes to obtained slices.

#[test]
fn write_all_three_buffers() {
    let (mut slice, _) = make_slice(
        [FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER],
        0..FIRST_BUFFER.len() + SECOND_BUFFER.len() + THIRD_BUFFER.len(),
    );
    let mut iter = slice.iter_mut();

    let first_slice = iter.next().unwrap();
    assert_eq!(first_slice, [1, 2, 3, 4, 5].as_mut_slice());

    let second_slice = iter.next().unwrap();
    assert_eq!(second_slice, [6, 7, 8].as_mut_slice());

    let third_slice = iter.next().unwrap();
    assert_eq!(third_slice, [9, 10, 11].as_mut_slice());

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());

    first_slice.iter_mut().for_each(|item| *item += 1);
    second_slice.iter_mut().for_each(|item| *item += 1);
    third_slice.iter_mut().for_each(|item| *item += 1);

    let mut iter = slice.iter_mut();

    assert_eq!(iter.next(), Some([2, 3, 4, 5, 6].as_mut_slice()));
    assert_eq!(iter.next(), Some([7, 8, 9].as_mut_slice()));
    assert_eq!(iter.next(), Some([10, 11, 12].as_mut_slice()));

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}
