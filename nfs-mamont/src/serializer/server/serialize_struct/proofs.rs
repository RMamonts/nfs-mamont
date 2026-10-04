//! Kani harnesses for [`WriteBuffer`] record framing.

use std::io::{self, IoSlice};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::AsyncWrite;

use super::{Serializer, WriteBuffer};
use crate::allocator::Buffer;
use crate::consts::rpc::{HEADER_MASK, MAX_FRAGMENT_SIZE, RMS_HEADER_SIZE};
use crate::consts::xdr::ALIGNMENT;
use crate::nlm::cookie::Cookie;
use crate::nlm::procedures::test::{Nlm4TestReply, Nlm4TestRes};
use crate::nlm::{Nlm4Stats, NlmRes};
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::task::{ProcReply, ProcResult};

/// Writer that records everything written to it.
struct Recorder {
    written: Vec<u8>,
    /// Accept an arbitrary non-zero prefix of every write, as a socket may.
    partial: bool,
    /// Accept a write across several slices; otherwise behave like tokio's
    /// default `poll_write_vectored`, which writes the first non-empty slice.
    vectored: bool,
}

impl Recorder {
    fn new(partial: bool, vectored: bool) -> Self {
        Self { written: Vec::new(), partial, vectored }
    }

    /// Number of bytes to accept out of `available`.
    fn accept(&self, available: usize) -> usize {
        if self.partial {
            kani::any_where(|len| (1..=available).contains(len))
        } else {
            available
        }
    }
}

impl AsyncWrite for Recorder {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        let len = this.accept(buf.len());
        this.written.extend_from_slice(&buf[..len]);
        Poll::Ready(Ok(len))
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        if !self.vectored {
            let buf = bufs.iter().find(|buf| !buf.is_empty()).map_or(&[][..], |buf| &**buf);
            return self.poll_write(cx, buf);
        }
        let this = self.get_mut();
        let total: usize = bufs.iter().map(|buf| buf.len()).sum();
        if total == 0 {
            return Poll::Ready(Ok(0));
        }
        let accepted = this.accept(total);
        let mut left = accepted;
        for buf in bufs {
            let take = left.min(buf.len());
            this.written.extend_from_slice(&buf[..take]);
            left -= take;
        }
        Poll::Ready(Ok(accepted))
    }

    fn is_write_vectored(&self) -> bool {
        self.vectored
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// Payload made of arbitrary, possibly empty, chunks.
struct ChunkBuffer {
    chunks: Vec<Vec<u8>>,
}

impl Buffer for ChunkBuffer {
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
        self.chunks.iter().map(Vec::as_slice)
    }

    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
        self.chunks.iter_mut().map(Vec::as_mut_slice)
    }

    fn len(&self) -> usize {
        self.chunks.iter().map(Vec::len).sum()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn empty() -> Self {
        Self { chunks: Vec::new() }
    }
}

/// Record header for a last fragment of `size` bytes.
fn record_header(size: usize) -> [u8; RMS_HEADER_SIZE] {
    ((HEADER_MASK | size) as u32).to_be_bytes()
}

/// Every fragment size that fits into 31 bits is encoded as a last fragment;
/// larger sizes are refused.
#[kani::proof]
fn append_fragment_size_encodes_last_fragment() {
    let mut buffer = WriteBuffer::new(Recorder::new(false, false), RMS_HEADER_SIZE);
    let size: usize = kani::any();

    match buffer.append_fragment_size(size) {
        Ok(()) => {
            assert!(size <= MAX_FRAGMENT_SIZE);
            assert_eq!(buffer.buf[..RMS_HEADER_SIZE], record_header(size));
        }
        Err(_) => {
            assert!(size > MAX_FRAGMENT_SIZE);
        }
    }
}

/// Upper bound on staged reply bytes before the payload.
const MAX_STAGED: usize = 2;
/// Number of payload chunks.
const CHUNKS: usize = 2;
/// Upper bound on the length of one payload chunk.
const MAX_CHUNK: usize = 2;

/// However the socket splits the writes, a READ-style reply goes out as one
/// record: header, staged bytes, payload length, the first `count` payload
/// bytes and zero padding. The buffer is reset for the next reply.
fn check_send_inner_with_buffer(vectored: bool) {
    let mut buffer = WriteBuffer::new(Recorder::new(true, vectored), RMS_HEADER_SIZE);
    let staged_len: usize = kani::any_where(|len| *len <= MAX_STAGED);
    let staged: [u8; MAX_STAGED] = kani::any();
    buffer.buf.extend_from_slice(&staged[..staged_len]);

    let data: [[u8; MAX_CHUNK]; CHUNKS] = kani::any();
    let chunks: Vec<Vec<u8>> = data
        .iter()
        .map(|chunk| chunk[..kani::any_where(|len: &usize| *len <= MAX_CHUNK)].to_vec())
        .collect();
    let payload: Vec<u8> = chunks.concat();
    let count: usize = kani::any_where(|count| *count <= payload.len());

    kani::block_on(buffer.send_inner_with_buffer(ChunkBuffer { chunks }, count)).unwrap();

    let padding = (ALIGNMENT - count % ALIGNMENT) % ALIGNMENT;
    let mut expected = record_header(staged_len + 4 + count + padding).to_vec();
    expected.extend_from_slice(&staged[..staged_len]);
    expected.extend_from_slice(&(count as u32).to_be_bytes());
    expected.extend_from_slice(&payload[..count]);
    expected.resize(expected.len() + padding, 0);
    assert_eq!(buffer.socket.written, expected);
    assert_eq!(buffer.buf.len(), RMS_HEADER_SIZE);
}

#[kani::proof]
#[kani::unwind(20)]
fn send_inner_with_buffer_vectored_writer() {
    check_send_inner_with_buffer(true);
}

#[kani::proof]
#[kani::unwind(20)]
fn send_inner_with_buffer_plain_writer() {
    check_send_inner_with_buffer(false);
}

fn none_verifier() -> OpaqueAuth {
    OpaqueAuth { flavor: AuthFlavor::None, body: Vec::new() }
}

/// A reply that fails to serialize leaves nothing behind: the next reply goes
/// out as a record of its own.
///
/// No service produces such a reply today, since the reply types validate
/// their fields on construction; the harness guards the write task, which
/// keeps serving replies after a failure.
#[kani::proof]
#[kani::unwind(10)]
fn failed_reply_leaves_no_residue() {
    const FAILED_XID: u32 = 1;
    const NEXT_XID: u32 = 2;
    let mut serializer = Serializer::with_capacity(Recorder::new(false, true), 64);

    // `Denied` without a holder cannot be encoded.
    let failed = ProcReply::<ChunkBuffer> {
        xid: FAILED_XID,
        proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Test(Box::new(Nlm4TestRes {
            cookie: Cookie::new(0),
            test_stat: Nlm4TestReply { stat: Nlm4Stats::Denied, holder: None },
        }))))),
    };
    assert!(kani::block_on(serializer.form_reply(failed, none_verifier())).is_err());

    let next = ProcReply::<ChunkBuffer> {
        xid: NEXT_XID,
        proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Null))),
    };
    kani::block_on(serializer.form_reply(next, none_verifier())).unwrap();

    let written = &serializer.buffer.socket.written;
    let header = u32::from_be_bytes(written[..RMS_HEADER_SIZE].try_into().unwrap());
    assert_eq!(header as usize & MAX_FRAGMENT_SIZE, written.len() - RMS_HEADER_SIZE);
    assert_eq!(written[RMS_HEADER_SIZE..RMS_HEADER_SIZE + 4], NEXT_XID.to_be_bytes());
}
