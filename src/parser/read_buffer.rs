use std::cmp::min;
use std::io;
use std::io::{ErrorKind, Read, Write};

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

    pub(super) async fn fill_buffer(&mut self) -> io::Result<usize> {
        if self.buf.available_write() == 0 {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "Buffer exhausted"));
        }

        let bytes_read = self.socket.read(self.buf.write_slice()).await?;

        if bytes_read == 0 {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "Connection closed"));
        }

        self.buf.extend(bytes_read);
        self.total_bytes += bytes_read;
        Ok(bytes_read)
    }

    pub(super) async fn fill_exact(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        self.socket.read_exact(dest).await?;
        self.total_bytes += dest.len();
        Ok(dest.len())
    }

    pub(super) fn compact(&mut self) {
        self.buf.compact()
    }

    pub(super) fn read_to_dest(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        if dest.is_empty() {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "Buffer exhausted"));
        }

        let bytes_read = self.read(dest)?;

        if bytes_read == 0 {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "Connection closed"));
        }

        self.buf.extend(bytes_read);
        self.total_bytes += bytes_read;
        Ok(bytes_read)
    }

    pub(super) fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub(super) fn available_read(&self) -> usize {
        self.buf.available_read()
    }

    pub(super) fn clear(&mut self) {
        self.buf.clear();
    }
}

impl<S: AsyncRead + Unpin> Read for CountBuffer<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.buf.read(buf)?;
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
}

impl Read for ReadBuffer {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = min(buf.len(), self.available_read());
        buf[..len].copy_from_slice(&self.data[self.read_pos..self.read_pos + len]);
        self.consume(len);
        Ok(len)
    }
}

impl Write for ReadBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = min(buf.len(), self.available_write());
        self.write_slice()[..len].copy_from_slice(&buf[..len]);
        self.extend(len);
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
