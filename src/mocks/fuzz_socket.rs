use std::cmp::min;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

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

    // actually this method should be called inside poll_read, but it is async
    // that means, that attention should be paid, where it is used
    pub fn add_data(&mut self) {
        // not sure if we now need to check for Empty error
        // for fuzz test it's alright - we will do not more, than 2 blocks at a time
        while let Ok(new_data) = self.recv.try_recv() {
            self.data.copy_within(self.start..self.end, 0);
            self.end -= self.start;
            self.start = 0;
            let remaining = self.data.len() - self.end;
            if remaining < new_data.len() {
                // that branch shouldn't happen often, because it causes unbounded allocation
                let (left, right) = new_data.split_at(remaining);
                self.data[self.end..].copy_from_slice(left);
                self.data.extend_from_slice(right);
                self.end = self.data.len();
            } else {
                self.data[self.end..new_data.len()].copy_from_slice(&new_data);
                self.end += self.data.len();
            }
        }
    }
}

impl AsyncRead for FuzzMockSocket {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let inner = self.get_mut();
        if inner.end - inner.start == 0 {
            inner.add_data();
        }
        let remaining_data = inner.end - inner.start;
        let to_read = min(buf.remaining(), remaining_data);
        assert!(to_read > 0);
        if to_read > 0 {
            let start = inner.start;
            let end = inner.start + to_read;
            buf.put_slice(&inner.data[start..end]);
            inner.start += to_read;
        }
        Poll::Ready(Ok(()))
    }
}
