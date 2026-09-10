use std::cmp::min;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

const SEPARATE: usize = 50;

pub struct FuzzMockSocket {
    data: Vec<u8>,
    start: usize,
    end: usize,
    recv: UnboundedReceiver<Vec<u8>>,
}

pub struct FuzzSocketHandler {
    sender: UnboundedSender<Vec<u8>>,
}

impl FuzzSocketHandler {
    pub fn send_data(&mut self, data: Vec<u8>) {
        self.sender.send(data).unwrap();
    }
}

impl FuzzMockSocket {
    pub fn new() -> (Self, FuzzSocketHandler) {
        let (sender, recv) = mpsc::unbounded_channel();
        (FuzzMockSocket { data: Vec::new(), start: 0, end: 0, recv }, FuzzSocketHandler { sender })
    }

    pub fn with_data(data: Vec<u8>) -> (Self, FuzzSocketHandler) {
        let (sender, recv) = mpsc::unbounded_channel();
        (FuzzMockSocket { data, start: 0, end: 0, recv }, FuzzSocketHandler { sender })
    }
}

impl AsyncRead for FuzzMockSocket {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let inner = self.get_mut();
        if inner.end - inner.start == 0 {
            // just in case
            inner.start = 0;
            inner.end = 0;
            // necessary for buffer not to grow unconditionally
            inner.data.clear();
            // not sure if we now need to check for Empty error
            // for fuzz test it's alright - we will do not more, than 2 blocks at a time
            loop {
                match Pin::new(&mut inner.recv).poll_recv(cx) {
                    Poll::Ready(Some(new_data)) => {
                        inner.data.extend_from_slice(&new_data);
                        inner.end = inner.data.len();
                    }
                    _ => break,
                }
            }
        }
        let remaining_data = inner.end - inner.start;
        if remaining_data == 0 {
            return Poll::Pending;
        }
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