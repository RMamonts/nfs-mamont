use std::cmp::min;
use std::io::Write;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};

const DEFAULT_CAPACITY: usize = 4000;
const SEPARATE: usize = 15;

pub struct FuzzMockSocket {
    data: Vec<u8>,
    start: usize,
    end: usize,
}

impl FuzzMockSocket {
    pub fn new() -> Self {
        FuzzMockSocket { data: vec![0u8; DEFAULT_CAPACITY], start: 0, end: 0 }
    }
}

impl AsyncRead for FuzzMockSocket {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let inner = self.get_mut();
        let remaining_data = inner.end - inner.start;

        let to_read = min(SEPARATE, min(buf.remaining(), remaining_data));

        if to_read > 0 {
            let start = inner.start;
            let end = inner.start + to_read;
            buf.put_slice(&inner.data[start..end]);

            inner.start += to_read;
        }

        Poll::Ready(Ok(()))
    }
}

impl Write for FuzzMockSocket {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.data.copy_within(self.start..self.end, 0);
        self.end -= self.start;
        self.start = 0;
        // should be changed to something meaningful if several messages are written at ones
        let amount = min(buf.len(), self.data.len() - self.end);
        //println!("{}, {} {} {}", self.end + amount, amount, self.end, self.data.len());
        self.data[self.end..self.end + amount].copy_from_slice(&buf[..amount]);
        self.end += buf.len();
        Ok(amount)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
