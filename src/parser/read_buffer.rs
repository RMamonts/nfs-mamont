//! Buffered reading utilities for parsing XDR-encoded RPC messages.
//!
//! This module provides a double-buffered reader that efficiently handles
//! asynchronous reading from network sockets while supporting retry logic
//! for parsing operations that may require additional data.
//!
//! The main component is [`CountBuffer`], which wraps an [`AsyncRead`] stream
//! and provides a [`Read`] interface for synchronous parsing functions. It uses
//! two internal buffers to allow reading new data while
//! still being able to retry parsing from a previous position if needed.

use std::cmp::min;
use std::io;
use std::io::{ErrorKind, Read};

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::parser::{Error, Result};

/// A buffered reader that wraps an async stream and provides synchronous reading
/// with retry capability.
///
/// `CountBuffer` uses a double-buffering strategy with two internal [`ReadBuffer`]
/// instances. This allows the parser to:
/// - Read new data from the socket into one buffer while parsing from another
/// - Retry parsing operations by resetting read positions when more data is needed
/// - Track the total number of bytes consumed from the stream
///
/// The buffer implements [`Read`] to work with synchronous parsing functions,
/// while internally managing asynchronous I/O operations.
///
/// # Example
///
/// ```no_run
/// use tokio::io::AsyncRead;
/// use crate::parser::read_buffer::CountBuffer;
///
/// # async fn example<S: AsyncRead + Unpin>(socket: S) {
/// let mut buffer = CountBuffer::new(4096, socket);
/// // Use parse_with_retry to parse XDR-encoded data
/// # }
/// ```
pub struct CountBuffer<S: AsyncRead + Unpin> {
    // actually, there are definitely two
    bufs: Vec<ReadBuffer>,
    read: usize,
    write: usize,
    retry_mode: bool,
    socket: S,
    total_bytes: usize,
}

impl<S: AsyncRead + Unpin> CountBuffer<S> {
    /// Creates a new `CountBuffer` with the specified capacity for each internal buffer.
    ///
    /// # Arguments
    ///
    /// * `capacity` - The size in bytes for each of the two internal buffers
    /// * `socket` - The async stream to read from
    pub(super) fn new(capacity: usize, socket: S) -> CountBuffer<S> {
        Self {
            bufs: vec![ReadBuffer::new(capacity), ReadBuffer::new(capacity)],
            read: 0,
            write: 1,
            retry_mode: false,
            socket,
            total_bytes: 0,
        }
    }

    /// Fills the write buffer by reading data from the socket.
    ///
    /// This method reads available data from the async stream into the current
    /// write buffer. It returns the number of bytes read, or an error if the
    /// connection is closed or an I/O error occurs.
    ///
    /// Returns `Ok(0)` if the write buffer is full and no more data can be read.
    async fn fill_internal(&mut self) -> io::Result<usize> {
        if self.bufs[self.write].available_write() == 0 {
            return Ok(0);
        }

        let bytes_read = self.socket.read(self.bufs[self.write].write_slice()).await?;
        if bytes_read == 0 {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "Connection closed"));
        }

        self.bufs[self.write].extend(bytes_read);

        Ok(bytes_read)
    }

    /// Parses a value using the provided parsing function, with automatic retry on EOF.
    ///
    /// This method attempts to parse a value using a synchronous parsing function.
    /// If the parsing function encounters an `UnexpectedEof` error, this method will:
    /// 1. Read more data from the socket into the write buffer
    /// 2. Reset read positions to allow retrying from the same point
    /// 3. Recursively retry the parsing operation
    ///
    /// After successful parsing, if retry mode was used, the buffers are swapped
    /// to prepare for the next parsing operation.
    ///
    /// # Arguments
    ///
    /// * `caller` - A function that takes a `&mut dyn Read` and returns a `Result<T>`
    ///
    /// # Returns
    ///
    /// Returns the parsed value, or an error if parsing fails or I/O errors occur.
    pub async fn parse_with_retry<T>(
        &mut self,
        caller: impl Fn(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let retry_start_read = self.bufs[self.read].bytes_read();
        let retry_start_write = self.bufs[self.write].bytes_read();
        let retry_total = self.total_bytes;
        // there is no need to check if we reach end of buffer while appending data to buffer since we have buffer, that would
        // definitely be enough to read what we are planning
        match caller(self) {
            Err(Error::IO(err)) if err.kind() == ErrorKind::UnexpectedEof => {
                self.retry_mode = true;
                // called whenever we need to read more data
                match self.fill_internal().await {
                    Ok(0) => {
                        // it is impossible scenario?
                        Err(Error::IO(err))
                    }
                    Ok(_) => {
                        self.bufs[self.read].reset_read(retry_start_read);
                        self.bufs[self.write].reset_read(retry_start_write);
                        self.total_bytes = retry_total;
                        Box::pin(self.parse_with_retry(caller)).await
                    }
                    Err(e) => Err(Error::IO(e)),
                }
            }
            Ok(val) => {
                if self.retry_mode {
                    self.bufs[self.read].clean();
                    self.write = (self.write + 1) % 2;
                    self.read = (self.read + 1) % 2;
                }
                if self.read == self.write {
                    return Err(Error::IO(io::Error::other(
                        "Cannot read and write to one buffer simultaneously",
                    )));
                }
                self.retry_mode = false;
                Ok(val)
            }
            Err(err) => Err(err),
        }
    }

    /// Reads an exact number of bytes from the socket into the provided buffer.
    ///
    /// This method uses `read_exact` to ensure the entire buffer is filled.
    /// The total byte count is updated to reflect the bytes read.
    ///
    /// # Arguments
    ///
    /// * `dest` - The destination buffer to fill
    ///
    /// # Returns
    ///
    /// Returns the number of bytes read (equal to `dest.len()`), or an error
    /// if the connection is closed before the buffer can be filled.
    pub(super) async fn read_from_async(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        self.socket.read_exact(dest).await?;
        self.total_bytes += dest.len();
        Ok(dest.len())
    }

    /// Resets the total byte counter to zero.
    ///
    /// This is typically called after successfully parsing a complete message
    /// to prepare for parsing the next message.
    pub(super) fn clean(&mut self) {
        self.total_bytes = 0;
    }

    /// Reads data from the internal buffers into the provided destination buffer.
    ///
    /// This method reads from internal buffers,
    /// filling the destination buffer with available data.
    ///
    /// # Arguments
    ///
    /// * `dest` - The destination buffer to fill
    ///
    /// # Returns
    ///
    /// Returns the number of bytes read, which may be less than `dest.len()`
    /// if not enough data is available in the internal buffers.
    pub(super) fn read_from_inner(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        if dest.is_empty() {
            return Ok(0);
        }
        self.read(dest)
    }

    /// Returns the total number of bytes consumed from the stream since the last reset.
    ///
    /// This count includes all bytes that have been read from the socket,
    /// whether they were consumed by parsing operations or discarded.
    pub(super) fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Discards the specified number of bytes from the stream.
    ///
    /// This method is used to skip over unparsed or unwanted data. It first
    /// consumes available bytes from the internal buffers, then reads and discards
    /// any remaining bytes directly from the socket.
    ///
    /// # Arguments
    ///
    /// * `n` - The number of bytes to discard
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if exactly `n` bytes were discarded, or an error if
    /// the connection is closed before all bytes can be discarded.
    pub(super) async fn discard_bytes(&mut self, n: usize) -> io::Result<()> {
        let from_inner = {
            let from_inner1 = min(self.bufs[self.read].available_read(), n);
            self.bufs[self.read].consume(from_inner1);
            let from_inner2 = min(self.bufs[self.write].available_read(), n - from_inner1);
            self.bufs[self.write].consume(from_inner2);
            from_inner1 + from_inner2
        };

        self.total_bytes += from_inner;
        let from_socket = n - from_inner;
        if from_socket == 0 {
            return Ok(());
        }

        let mut src = (&mut self.socket).take(from_socket as u64);

        let mut actual = 0;

        loop {
            let n = src.read(self.bufs[self.write].write_slice()).await?;
            if n == 0 {
                break;
            }
            actual += n;
        }

        self.total_bytes += actual;

        // probably useless, since we have guarantees from Take
        if actual != from_socket {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "Discarded not valid amount of bytes",
            ));
        }

        Ok(())
    }
}

impl<S: AsyncRead + Unpin> Read for CountBuffer<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n1 = self.bufs[self.read].read(buf)?;
        let n2 = self.bufs[self.write].read(&mut buf[n1..])?;
        self.total_bytes += n1 + n2;
        Ok(n1 + n2)
    }
}

/// An internal buffer for managing read and write positions.
///
/// `ReadBuffer` maintains a fixed-size buffer with separate read and write
/// positions, allowing efficient tracking of available data and available space.
/// It implements [`Read`] to provide a standard interface for consuming data.
struct ReadBuffer {
    data: Vec<u8>,
    read_pos: usize,
    write_pos: usize,
}

#[allow(dead_code)]
impl ReadBuffer {
    /// Creates a new `ReadBuffer` with the specified capacity.
    ///
    /// The buffer is initialized with zeros and both read and write positions
    /// start at the beginning.
    fn new(capacity: usize) -> Self {
        Self { data: vec![0u8; capacity], read_pos: 0, write_pos: 0 }
    }

    /// Returns the current read position (number of bytes consumed).
    fn bytes_read(&self) -> usize {
        self.read_pos
    }

    /// Returns the number of bytes available to read.
    ///
    /// This is the difference between the write position and read position.
    fn available_read(&self) -> usize {
        self.write_pos - self.read_pos
    }

    /// Returns the number of bytes available for writing.
    ///
    /// This is the remaining space in the buffer from the write position to the end.
    fn available_write(&self) -> usize {
        self.data.len() - self.write_pos
    }

    /// Returns a mutable slice of the buffer starting from the write position.
    ///
    /// This slice can be used to write data directly into the buffer.
    fn write_slice(&mut self) -> &mut [u8] {
        &mut self.data[self.write_pos..]
    }

    /// Advances the read position by `n` bytes, consuming that many bytes.
    ///
    /// # Arguments
    ///
    /// * `n` - The number of bytes to consume
    fn consume(&mut self, n: usize) {
        self.read_pos += n;
    }

    /// Advances the write position by `n` bytes, indicating that data has been written.
    ///
    /// # Arguments
    ///
    /// * `n` - The number of bytes that were written
    fn extend(&mut self, n: usize) {
        self.write_pos += n;
    }

    /// Resets the read position to a specific value.
    ///
    /// This is used during retry operations to allow re-reading from a previous position.
    ///
    /// # Arguments
    ///
    /// * `n` - The new read position
    fn reset_read(&mut self, n: usize) {
        self.read_pos = n;
    }

    /// Resets both read and write positions to zero, clearing the buffer.
    ///
    /// This prepares the buffer for reuse after a complete message has been processed.
    fn clean(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
    }
}

impl Read for ReadBuffer {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = min(buf.len(), self.available_read());
        buf[..len].copy_from_slice(&self.data[self.read_pos..self.read_pos + len]);
        self.consume(len);
        Ok(len)
    }
}
