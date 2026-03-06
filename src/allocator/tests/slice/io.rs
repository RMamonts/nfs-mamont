//! Defines tests for [`crate::allocator::Slice`] stream I/O helpers.

use std::io::{self, Read as _, Write as _};

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
        result.push(buf.into_boxed_slice());
    }

    (Slice::new(result, range, sender), receiver)
}

#[test]
fn len_and_is_empty_follow_visible_range() {
    let (slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER], 2..6);
    assert_eq!(slice.len(), 4);
    assert!(!slice.is_empty());

    let (empty, _) = make_slice([FIRST_BUFFER], 3..3);
    assert_eq!(empty.len(), 0);
    assert!(empty.is_empty());
}

#[test]
fn reader_reads_visible_range_across_buffers() {
    let (slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER], 2..9);

    let mut reader = slice.reader();
    assert_eq!(reader.remaining(), 7);

    let mut output = Vec::new();
    reader.read_to_end(&mut output).unwrap();

    assert_eq!(output, vec![3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn reader_supports_partial_reads_without_affecting_iter() {
    let (slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER], 1..10);

    let mut reader = slice.reader();
    let mut chunk = [0; 4];
    assert_eq!(reader.read(&mut chunk).unwrap(), 4);
    assert_eq!(chunk, [2, 3, 4, 5]);
    assert_eq!(reader.remaining(), 5);

    assert_eq!(reader.read(&mut chunk).unwrap(), 4);
    assert_eq!(chunk, [6, 7, 8, 9]);
    assert_eq!(reader.remaining(), 1);

    assert_eq!(reader.read(&mut chunk[..1]).unwrap(), 1);
    assert_eq!(chunk[0], 10);
    assert_eq!(reader.remaining(), 0);
    assert_eq!(reader.read(&mut chunk).unwrap(), 0);

    let content = slice.iter().collect::<Vec<_>>();
    assert_eq!(content, vec![&[2, 3, 4, 5][..], &[6, 7, 8][..], &[9, 10][..]]);
}

#[test]
fn writer_writes_visible_range_across_buffers() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER], 2..9);

    let mut writer = slice.writer();
    assert_eq!(writer.remaining(), 7);
    writer.write_all(b"abcdefg").unwrap();
    assert_eq!(writer.remaining(), 0);

    let content = slice.iter().collect::<Vec<_>>();
    assert_eq!(content, vec![&b"abc"[..], &b"def"[..], &b"g"[..]]);
}

#[test]
fn writer_supports_partial_writes_and_new_writer_restarts_from_beginning() {
    let (mut slice, _) = make_slice([FIRST_BUFFER, SECOND_BUFFER, THIRD_BUFFER], 1..8);

    {
        let mut writer = slice.writer();
        assert_eq!(writer.write(b"wxyz").unwrap(), 4);
        assert_eq!(writer.remaining(), 3);
        assert_eq!(writer.write(b"12").unwrap(), 2);
        assert_eq!(writer.remaining(), 1);
    }

    let content = slice.iter().collect::<Vec<_>>();
    assert_eq!(content, vec![&b"wxyz"[..], &[49, 50, 8][..]]);

    {
        let mut writer = slice.writer();
        writer.write_all(b"ABCDEFG").unwrap();
        assert_eq!(writer.write(b"!").unwrap(), 0);
        let error = writer.write_all(b"!").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    }

    let content = slice.iter().collect::<Vec<_>>();
    assert_eq!(content, vec![&b"ABCD"[..], &b"EFG"[..]]);
}

#[test]
fn empty_range_reader_and_writer_are_noops() {
    let (mut slice, _) = make_slice([FIRST_BUFFER], 2..2);

    let mut reader = slice.reader();
    let mut buffer = [7; 3];
    assert_eq!(reader.read(&mut buffer).unwrap(), 0);
    assert_eq!(buffer, [7; 3]);
    assert_eq!(reader.remaining(), 0);

    let mut writer = slice.writer();
    assert_eq!(writer.write(b"abc").unwrap(), 0);
    assert_eq!(writer.remaining(), 0);

    let content = slice.iter().collect::<Vec<_>>();
    assert!(content.is_empty());
}
