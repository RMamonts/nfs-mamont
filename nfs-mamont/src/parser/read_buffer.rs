//! Buffered frame reading for parsing XDR-encoded RPC messages.
//!
//! This module provides [`FrameReader`] --- a buffered reader that exposes the
//! bodies of RMS frames (RFC 5531, section 11) of an async stream to
//! synchronous parsing code.
//!
//! The reader guarantees that after [`FrameReader::begin_body`] the first
//! `min(frame_size, capacity)` bytes of the frame body (the "head window") are
//! buffered in memory, so procedure arguments are parsed synchronously via the
//! [`Read`] implementation without any retries or awaits. Only opaque payloads
//! that do not fit into the head window (NFSv3 `WRITE` data) are streamed from
//! the socket directly via [`FrameReader::read_body_exact_vectored`].
//!
//! Synchronous reads never cross the frame boundary: [`Read::read`] is limited
//! by the number of unconsumed frame body bytes, so a parser can never consume
//! bytes of the next frame. Bytes of the next frame that were buffered while
//! refilling are consumed by the next [`FrameReader::read_frame_header`] call.

use std::cmp::min;
use std::io::{self, ErrorKind, IoSliceMut, Read};

use tokio::io::AsyncReadExt;
use tokio::net::tcp::OwnedReadHalf;

use crate::allocator::Buffer;
use crate::consts::rpc::RMS_HEADER_SIZE;
use crate::consts::xdr::ALIGNMENT;

/// A buffered reader exposing RMS frame bodies of an async stream.
///
/// The internal buffer holds a sliding window of unconsumed stream bytes.
/// Invariant: direct socket reads ([`FrameReader::read_body_exact_vectored`],
/// [`FrameReader::discard_body`]) are only performed when the buffer is empty,
/// so stream bytes are always consumed in order.
pub struct FrameReader {
    socket: OwnedReadHalf,
    buf: Vec<u8>,
    /// Start of the unconsumed bytes in `buf`.
    start: usize,
    /// End of the valid bytes in `buf`.
    end: usize,
    /// Bytes of the current frame body not yet consumed.
    frame_remaining: usize,
}

impl FrameReader {
    /// Creates a new `FrameReader` with the given buffer capacity.
    ///
    /// The capacity bounds the head window of a frame: the arguments of every
    /// procedure (except opaque `WRITE` data, which is streamed) must fit into
    /// `capacity` bytes, otherwise parsing fails with `InvalidData`.
    ///
    /// # Panics
    ///
    /// If `capacity` is less than the RMS header size (4 bytes).
    pub fn new(capacity: usize, socket: OwnedReadHalf) -> FrameReader {
        assert!(capacity >= RMS_HEADER_SIZE, "capacity must hold at least an RMS header");
        Self { socket, buf: vec![0u8; capacity], start: 0, end: 0, frame_remaining: 0 }
    }

    /// Number of buffered unconsumed bytes.
    #[inline]
    fn buffered(&self) -> usize {
        self.end - self.start
    }

    /// Number of unconsumed bytes of the current frame body.
    #[inline]
    pub fn frame_remaining(&self) -> usize {
        self.frame_remaining
    }

    /// Reads more data from the socket into the buffer, compacting it first
    /// if the write area is exhausted.
    async fn fill(&mut self) -> io::Result<usize> {
        if self.end == self.buf.len() {
            self.buf.copy_within(self.start..self.end, 0);
            self.end -= self.start;
            self.start = 0;
        }
        let bytes_read = self.socket.read(&mut self.buf[self.end..]).await?;
        if bytes_read == 0 {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "Connection closed"));
        }
        self.end += bytes_read;
        Ok(bytes_read)
    }

    /// Fills the buffer until at least `n` unconsumed bytes are available.
    ///
    /// `n` must not exceed the buffer capacity.
    async fn ensure_buffered(&mut self, n: usize) -> io::Result<()> {
        debug_assert!(n <= self.buf.len());
        while self.buffered() < n {
            self.fill().await?;
        }
        Ok(())
    }

    /// Reads the 4-byte RMS frame header from the stream.
    ///
    /// The header is not part of the frame body and is not accounted for in
    /// [`FrameReader::frame_remaining`].
    pub async fn read_frame_header(&mut self) -> io::Result<u32> {
        self.ensure_buffered(RMS_HEADER_SIZE).await?;
        let bytes: [u8; RMS_HEADER_SIZE] =
            self.buf[self.start..self.start + RMS_HEADER_SIZE].try_into().unwrap();
        self.start += RMS_HEADER_SIZE;
        Ok(u32::from_be_bytes(bytes))
    }

    /// Starts a frame body of `size` bytes and buffers its head window
    /// (`min(size, capacity)` bytes), so subsequent synchronous parsing does
    /// not run out of data before the window is exhausted.
    pub async fn begin_body(&mut self, size: usize) -> io::Result<()> {
        self.frame_remaining = size;
        let head = min(size, self.buf.len());
        self.ensure_buffered(head).await
    }

    /// Reads exactly `count` bytes of the frame body into the front of a
    /// [`Buffer`], followed by its `(ALIGNMENT - count % ALIGNMENT) % ALIGNMENT`
    /// trailing padding bytes, which are discarded.
    ///
    /// The buffered head window is consumed first and the remainder is streamed
    /// directly from the socket via vectored I/O: the `count` payload bytes are
    /// laid out sequentially across the buffer chunks and the padding is read
    /// into scratch space, coalescing the socket reads into a single `readv`
    /// syscall per readiness notification where possible (mirroring the payload
    /// handling in the serializer). Used for opaque payloads (NFSv3 `WRITE`
    /// data) that may not fit into the buffered head window.
    ///
    /// Both the data and its padding must lie within the current frame.
    pub async fn read_body_exact_vectored<B: Buffer>(
        &mut self,
        buffer: &mut B,
        count: usize,
    ) -> io::Result<()> {
        let padding = (ALIGNMENT - count % ALIGNMENT) % ALIGNMENT;
        let total = count + padding;
        if total > self.frame_remaining {
            return Err(io::Error::new(ErrorKind::InvalidData, "Read beyond frame bounds"));
        }

        const PADDING_BYTES: [u8; ALIGNMENT] = [0u8; ALIGNMENT];
        let mut padding_buf = PADDING_BYTES;

        let mut consumed = 0;

        // Copy what is already buffered in the head window
        let from_window = min(total, self.buffered());
        if from_window > 0 {
            let payload_from_window = min(from_window, count);
            if payload_from_window > 0 {
                let mut left = payload_from_window;
                for chunk in buffer.chunks_mut() {
                    if left == 0 {
                        break;
                    }
                    let take = min(chunk.len(), left);
                    chunk[..take].copy_from_slice(&self.buf[self.start..self.start + take]);
                    self.start += take;
                    left -= take;
                }
            }
            self.start += from_window - payload_from_window;
            consumed += from_window;
            self.frame_remaining -= from_window;
        }

        let to_read = total - from_window;
        let mut socket_read = 0;
        while socket_read < to_read {
            let mut to_skip = consumed + socket_read;
            let mut iov = Vec::new();
            let mut payload_left = count;
            for chunk in buffer.chunks_mut() {
                if payload_left == 0 {
                    break;
                }
                let take = payload_left.min(chunk.len());
                let chunk = &mut chunk[..take];
                payload_left -= take;
                if to_skip >= chunk.len() {
                    to_skip -= chunk.len();
                } else if to_skip > 0 {
                    iov.push(IoSliceMut::new(&mut chunk[to_skip..]));
                    to_skip = 0;
                } else if !chunk.is_empty() {
                    iov.push(IoSliceMut::new(chunk));
                }
            }
            if to_skip < padding {
                iov.push(IoSliceMut::new(&mut padding_buf[to_skip..padding]));
            }

            self.socket.readable().await?;
            match self.socket.try_read_vectored(&mut iov) {
                Ok(n) if n > 0 => socket_read += n,
                Ok(_) => {
                    return Err(io::Error::new(ErrorKind::UnexpectedEof, "Connection closed"));
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e),
            }
        }
        self.frame_remaining -= to_read;

        Ok(())
    }

    /// Discards `n` bytes of the frame body, first from the buffer, then from
    /// the socket (reusing the buffer as scratch space).
    pub async fn discard_body(&mut self, n: usize) -> io::Result<()> {
        if n > self.frame_remaining {
            return Err(io::Error::new(ErrorKind::InvalidData, "Discard beyond frame bounds"));
        }
        let from_buf = min(n, self.buffered());
        self.start += from_buf;
        self.frame_remaining -= from_buf;

        let mut left = n - from_buf;
        if left > 0 {
            // left > 0 implies the buffer is empty; reuse it as scratch space.
            self.start = 0;
            self.end = 0;
            while left > 0 {
                let take = min(left, self.buf.len());
                self.socket.read_exact(&mut self.buf[..take]).await?;
                left -= take;
                self.frame_remaining -= take;
            }
        }
        Ok(())
    }

    /// Discards all unconsumed bytes of the current frame body, aligning the
    /// stream to the next frame after a protocol-level error.
    pub async fn discard_rest_of_frame(&mut self) -> io::Result<()> {
        let remaining = self.frame_remaining;
        self.discard_body(remaining).await
    }
}

impl Read for FrameReader {
    /// Reads buffered frame body bytes.
    ///
    /// The read is limited both by the buffered data and by the frame
    /// boundary; `Ok(0)` with a non-empty `dest` means either the frame body
    /// is fully consumed or the head window is exhausted (arguments larger
    /// than the buffer).
    fn read(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        let n = min(dest.len(), min(self.buffered(), self.frame_remaining));
        dest[..n].copy_from_slice(&self.buf[self.start..self.start + n]);
        self.start += n;
        self.frame_remaining -= n;
        Ok(n)
    }
}
