use std::cmp::min;
use std::io;
use std::io::Read;

use tokio::io::{AsyncRead, AsyncReadExt};

// object to manage all read requests
// we count bytes read totally
pub struct CountBuffer<S: AsyncRead + Unpin> {
    buf: ReadBuffer,
    socket: S,
    total_bytes: usize,
}

impl<S: AsyncRead + Unpin> CountBuffer<S> {
    pub(super) fn new(capacity: usize, socket: S) -> CountBuffer<S> {
        Self { buf: ReadBuffer::new(capacity), socket, total_bytes: 0 }
    }

    pub(super) async fn fill_buffer(&mut self, len: usize) -> io::Result<usize> {
        let n = self.socket.read_exact(self.buf.write_slice(len)).await?;
        self.buf.extend(n);
        Ok(n)
    }

    pub(super) fn read_to_dest(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.buf.read(buf)?;
        self.total_bytes += n;
        Ok(n)
    }

    pub(super) async fn fill_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.socket.read_exact(buf).await?;
        self.total_bytes += buf.len();
        Ok(())
    }

    pub(super) fn available_read(&self) -> usize {
        self.buf.available_read()
    }

    pub(super) fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub(super) fn clean(&mut self) {
        self.buf.clear();
        self.total_bytes = 0;
    }
}

impl<S: AsyncRead + Unpin> Read for CountBuffer<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.buf.read(buf)?;
        self.total_bytes += n;
        Ok(n)
    }
}

// object to manage static buffer
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
    fn available_read(&self) -> usize {
        self.write_pos - self.read_pos
    }

    fn write_slice(&mut self, max: usize) -> &mut [u8] {
        let size = self.data.len();
        &mut self.data[self.write_pos..min(self.write_pos + max, size)]
    }

    fn consume(&mut self, n: usize) {
        self.read_pos += n;
    }

    fn extend(&mut self, n: usize) {
        self.write_pos += n;
    }

    fn clear(&mut self) {
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
