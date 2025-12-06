use crate::parser::{Error, Result};
use std::cmp::min;
use std::io;
use std::io::{ErrorKind, Read};
use tokio::io::{AsyncRead, AsyncReadExt};

pub struct CountBuffer<S: AsyncRead + Unpin> {
    buf: ReadBuffer,
    socket: S,
    total_bytes: usize,
}

impl<S: AsyncRead + Unpin> CountBuffer<S> {
    pub(super) fn new(capacity: usize, socket: S) -> CountBuffer<S> {
        Self { buf: ReadBuffer::new(capacity), socket, total_bytes: 0 }
    }

    // this one is critically needed in continuous reading to internal buffer
    // read into inner buffer from socket
    pub(super) async fn fill_internal(&mut self) -> io::Result<usize> {
        if self.buf.available_write() == 0 {
            return Ok(0);
        }

        let bytes_read = self.socket.read(self.buf.write_slice()).await?;

        if bytes_read == 0 {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "Connection closed"));
        }

        self.buf.extend(bytes_read);
        Ok(bytes_read)
    }

    pub async fn parse_with_retry<T>(
        &mut self,
        caller: impl Fn(&mut dyn Read) -> Result<T>,
    ) -> Result<T> {
        let retry_start = self.buf.bytes_read();
        let retry_total = self.total_bytes;
        // there is no need to check if we reach end of buffer while appending data to buffer since we have buffer, that would
        // definitely be enough to read what we are planning
        match caller(self) {
            Err(Error::IO(err)) if err.kind() == ErrorKind::UnexpectedEof => {
                // called whenever we need to read more data
                match self.fill_internal().await {
                    Ok(0) => Err(Error::IO(err)),
                    Ok(_) => {
                        self.buf.reset_read(retry_start);
                        self.total_bytes = retry_total;
                        Box::pin(self.parse_with_retry(caller)).await
                    }
                    Err(e) => Err(Error::IO(e)),
                }
            }
            result => result,
        }
    }

    // read exact (fill exact amount) to internal buffer
    // read_exact from socket to external buffer
    pub(super) async fn read_from_async(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        self.socket.read_exact(dest).await?;
        self.total_bytes += dest.len();
        Ok(dest.len())
    }

    pub(super) fn clean(&mut self) {
        self.buf.compact();
        self.total_bytes = 0;
    }

    // read from inner to external
    pub(super) fn read_from_inner(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        if dest.is_empty() {
            return Ok(0);
        }

        let bytes_read = self.read(dest)?;

        if bytes_read == 0 {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "Connection closed"));
        }

        self.buf.extend(bytes_read);
        Ok(bytes_read)
    }

    pub(super) fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub(super) fn available_read(&self) -> usize {
        self.buf.available_read()
    }

    pub(super) async fn discard_bytes(&mut self, n: usize) -> io::Result<()> {
        let from_inner = self.buf.available_read();
        self.buf.consume(from_inner);
        self.total_bytes += from_inner;
        let from_socket = n - from_inner;

        if from_socket == 0 {
            return Ok(());
        }

        let mut src = (&mut self.socket).take(from_socket as u64);

        let mut dest = tokio::io::sink();
        let actual = tokio::io::copy(&mut src, &mut dest).await?;

        self.total_bytes += actual as usize;

        if actual != from_socket as u64 {
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
        let n = self.buf.read(buf)?;
        // apparently, that line is problem: maybe do increment that part somehow in one place?
        // move that increment from here and do it explicitly
        self.total_bytes += n;
        Ok(n)
    }
}

struct ReadBuffer {
    data: Vec<u8>,
    read_pos: usize,
    write_pos: usize,
}

#[allow(dead_code)]
impl ReadBuffer {
    fn new(capacity: usize) -> Self {
        Self { data: vec![0u8; capacity], read_pos: 0, write_pos: 0 }
    }

    fn bytes_read(&self) -> usize {
        self.read_pos
    }
    fn available_read(&self) -> usize {
        self.write_pos - self.read_pos
    }

    fn available_write(&self) -> usize {
        self.data.len() - self.write_pos
    }

    fn read_slice(&self) -> &[u8] {
        &self.data[self.read_pos..self.write_pos]
    }

    fn write_slice(&mut self) -> &mut [u8] {
        &mut self.data[self.write_pos..]
    }

    fn consume(&mut self, n: usize) {
        self.read_pos += n;
    }

    fn extend(&mut self, n: usize) {
        self.write_pos += n;
    }

    fn compact(&mut self) {
        if self.read_pos > 0 {
            self.data.copy_within(self.read_pos..self.write_pos, 0);
            self.write_pos -= self.read_pos;
            self.read_pos = 0;
        }
    }

    fn clear(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
    }

    fn reset_read(&mut self, n: usize) {
        self.read_pos = n;
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
