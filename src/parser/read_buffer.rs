use std::cmp::min;
use std::io;
use std::io::{ErrorKind, Read};

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::parser::{Error, Result};

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

    // this one is critically needed in continuous reading to internal buffer
    // read into inner buffer from socket
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

    pub async fn parse_with_retry<T>(
        &mut self,
        caller: impl Fn(&mut dyn Read) -> Result<T>,
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

    // read exact (fill exact amount) to internal buffer
    // read_exact from socket to external buffer
    pub(super) async fn read_from_async(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        self.socket.read_exact(dest).await?;
        self.total_bytes += dest.len();
        Ok(dest.len())
    }

    pub(super) fn clean(&mut self) {
        self.total_bytes = 0;
    }

    // read from inner to external
    pub(super) fn read_from_inner(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        if dest.is_empty() {
            return Ok(0);
        }
        self.read(dest)
    }

    pub(super) fn total_bytes(&self) -> usize {
        self.total_bytes
    }

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

    fn write_slice(&mut self) -> &mut [u8] {
        &mut self.data[self.write_pos..]
    }

    fn consume(&mut self, n: usize) {
        self.read_pos += n;
    }

    fn extend(&mut self, n: usize) {
        self.write_pos += n;
    }

    fn reset_read(&mut self, n: usize) {
        self.read_pos = n;
    }

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
