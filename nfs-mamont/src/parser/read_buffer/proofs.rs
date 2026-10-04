//! Kani harnesses for [`FrameReader`].
//!
//! The capacity is small and fixed; the reader does not depend on its value
//! beyond the `capacity >= RMS_HEADER_SIZE` precondition.

use std::io::Read;

use super::FrameReader;
use crate::consts::rpc::RMS_HEADER_SIZE;
use crate::parser::proofs::socket::MockSocket;

const CAPACITY: usize = 8;

/// A reader in an arbitrary state: any buffered window, any buffer contents
/// and any number of unconsumed frame body bytes.
///
/// Returns the reader and its buffer contents.
fn any_reader(socket: MockSocket) -> (FrameReader<MockSocket>, [u8; CAPACITY]) {
    let mut reader = FrameReader::new(CAPACITY, socket);
    let content: [u8; CAPACITY] = kani::any();
    reader.buf.copy_from_slice(&content);
    let end: usize = kani::any_where(|end| *end <= CAPACITY);
    let start: usize = kani::any_where(|start| *start <= end);
    reader.start = start;
    reader.end = end;
    reader.frame_remaining = kani::any();
    (reader, content)
}

/// From any state, a synchronous read returns as many bytes as the buffer,
/// the frame and the destination allow, in stream order, and keeps the window
/// consistent. Since the starting state is arbitrary, this holds after any
/// sequence of earlier calls.
#[kani::proof]
#[kani::unwind(10)]
fn read_stays_within_buffer_and_frame() {
    let (mut reader, content) = any_reader(MockSocket::whole(&[]));
    let (start, end, remaining) = (reader.start, reader.end, reader.frame_remaining);

    let mut dest = [0u8; CAPACITY + 1];
    let len: usize = kani::any_where(|len| *len <= CAPACITY + 1);
    let read = reader.read(&mut dest[..len]).unwrap();

    assert_eq!(read, len.min(end - start).min(remaining));
    assert_eq!(&dest[..read], &content[start..start + read]);
    assert_eq!(reader.start, start + read);
    assert_eq!(reader.end, end);
    assert_eq!(reader.frame_remaining, remaining - read);
}

/// From any state, refilling satisfies any request up to the capacity: it
/// never reads into an empty buffer (the socket asserts that), never reports
/// a false end of stream, and keeps the buffered bytes in stream order.
#[kani::proof]
#[kani::unwind(10)]
fn ensure_buffered_always_makes_room() {
    let incoming: [u8; CAPACITY] = kani::any();
    let (mut reader, content) = any_reader(MockSocket::segmented(&incoming));
    let (start, end) = (reader.start, reader.end);
    let wanted: usize = kani::any_where(|wanted| *wanted <= CAPACITY);

    kani::block_on(reader.ensure_buffered(wanted)).unwrap();

    let kept = end - start;
    assert!(reader.buffered() >= wanted);
    assert_eq!(&reader.buf[reader.start..reader.start + kept], &content[start..end]);
    let fresh = reader.buffered() - kept;
    assert_eq!(&reader.buf[reader.start + kept..reader.end], &incoming[..fresh]);
}

/// Frame body shorter than the capacity: the buffer also holds bytes of the
/// next frame.
const SHORT_BODY: usize = 3;
/// Frame body longer than the capacity: part of it is read from the socket
/// directly.
const LONG_BODY: usize = 10;
/// Stream of one frame with the longest body followed by the next frame header.
const STREAM: usize = RMS_HEADER_SIZE + LONG_BODY + RMS_HEADER_SIZE;

/// However the stream is segmented and however the body is split between a
/// buffered read, a direct read and a discard, the reader delivers the body
/// in order, buffers the head window up front, and leaves the stream at the
/// next frame header.
#[kani::proof]
#[kani::unwind(20)]
fn frame_body_is_delivered_in_order() {
    let stream: [u8; STREAM] = kani::any();
    let body_len = if kani::any() { SHORT_BODY } else { LONG_BODY };
    let body = &stream[RMS_HEADER_SIZE..RMS_HEADER_SIZE + body_len];
    let mut reader = FrameReader::new(CAPACITY, MockSocket::segmented(&stream));

    let header = kani::block_on(reader.read_frame_header()).unwrap();
    assert_eq!(header.to_be_bytes(), stream[..RMS_HEADER_SIZE]);
    kani::block_on(reader.begin_body(body_len)).unwrap();

    let mut buffered = [0u8; LONG_BODY];
    let wanted: usize = kani::any_where(|wanted| *wanted <= body_len);
    let got = reader.read(&mut buffered[..wanted]).unwrap();
    if wanted <= CAPACITY {
        // The head window was buffered by `begin_body`.
        assert_eq!(got, wanted);
    }
    assert!(got <= wanted);
    assert_eq!(&buffered[..got], &body[..got]);

    let mut direct = [0u8; LONG_BODY];
    let direct_len: usize = kani::any_where(|len| *len <= body_len - got);
    kani::block_on(reader.read_body_exact(&mut direct[..direct_len])).unwrap();
    assert_eq!(&direct[..direct_len], &body[got..got + direct_len]);

    kani::block_on(reader.discard_rest_of_frame()).unwrap();
    assert_eq!(reader.frame_remaining(), 0);

    let next = kani::block_on(reader.read_frame_header()).unwrap();
    let next_start = RMS_HEADER_SIZE + body_len;
    assert_eq!(next.to_be_bytes(), stream[next_start..next_start + RMS_HEADER_SIZE]);
}
