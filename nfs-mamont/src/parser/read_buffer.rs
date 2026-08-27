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
//! that do not fit into the head window (NFSv3 `WRITE` data) are read from the
//! socket directly via [`FrameReader::read_body_exact`].
//!
//! Synchronous reads never cross the frame boundary: [`Read::read`] is limited
//! by the number of unconsumed frame body bytes, so a parser can never consume
//! bytes of the next frame. Bytes of the next frame that were buffered while
//! refilling are consumed by the next [`FrameReader::read_frame_header`] call.

use std::cmp::min;
use std::io::{self, ErrorKind, Read};

use tokio::io::{AsyncRead, AsyncReadExt};

/// Size of an RMS frame header in bytes.
const RMS_HEADER_SIZE: usize = 4;

/// A buffered reader exposing RMS frame bodies of an async stream.
///
/// The internal buffer holds a sliding window of unconsumed stream bytes.
/// Invariant: direct socket reads ([`FrameReader::read_body_exact`],
/// [`FrameReader::discard_body`]) are only performed when the buffer is empty,
/// so stream bytes are always consumed in order.
pub struct FrameReader<S: AsyncRead + Unpin> {
    socket: S,
    buf: Vec<u8>,
    /// Start of the unconsumed bytes in `buf`.
    start: usize,
    /// End of the valid bytes in `buf`.
    end: usize,
    /// Bytes of the current frame body not yet consumed.
    frame_remaining: usize,
}

impl<S: AsyncRead + Unpin> FrameReader<S> {
    /// Creates a new `FrameReader` with the given buffer capacity.
    ///
    /// The capacity bounds the head window of a frame: the arguments of every
    /// procedure (except opaque `WRITE` data, which is streamed) must fit into
    /// `capacity` bytes, otherwise parsing fails with `InvalidData`.
    ///
    /// # Panics
    ///
    /// If `capacity` is less than the RMS header size (4 bytes).
    pub fn new(capacity: usize, socket: S) -> FrameReader<S> {
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
        let checked_n = min(n, self.buffered());
        while self.buffered() < checked_n {
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
        self.ensure_buffered(size).await
    }

    /// Reads exactly `dest.len()` bytes of the frame body, first from the
    /// buffer, then directly from the socket.
    ///
    /// Used for opaque payloads (NFSv3 `WRITE` data) that may not fit into
    /// the buffered head window.
    pub async fn read_body_exact(&mut self, dest: &mut [u8]) -> io::Result<()> {
        if dest.len() > self.frame_remaining {
            return Err(io::Error::new(ErrorKind::InvalidData, "Read beyond frame bounds"));
        }
        let from_buf = min(dest.len(), self.buffered());
        dest[..from_buf].copy_from_slice(&self.buf[self.start..self.start + from_buf]);
        self.start += from_buf;
        self.frame_remaining -= from_buf;

        let rest = &mut dest[from_buf..];
        if !rest.is_empty() {
            // from_buf < dest.len() implies the buffer is empty, so a direct
            // socket read preserves stream ordering.
            self.socket.read_exact(rest).await?;
            self.frame_remaining -= rest.len();
        }
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

impl<S: AsyncRead + Unpin> Read for FrameReader<S> {
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
