use std::cmp::min;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

const DEFAULT_CAPACITY: usize = 4000;
const SEPARATE: usize = 15;

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
        (
            FuzzMockSocket { data: vec![0u8; DEFAULT_CAPACITY], start: 0, end: 0, recv },
            FuzzSocketHandler { sender },
        )
    }

    // actually this method should be called inside poll_read, but it is async
    // that means, that attention should be paid, where it is used
    pub fn add_data(&mut self) {
        // not sure if we now need to check for Empty error
        let new_data = self.recv.try_recv().unwrap();
        // not sure that now it matters, since we send one message at a time, but probably latter we would change that
        if self.start < self.end {
            self.data.copy_within(self.start..self.end, 0);
        }
        self.start = 0;
        self.end = new_data.len();
        // not of any particular sense; fail would mean severe problems with logic of this mock
        assert!(new_data.len() < DEFAULT_CAPACITY);
        self.data[..new_data.len()].clone_from_slice(&new_data);
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
        // why does it work only without SEPARATE???
        // there are no bugs in serialization/deserialization, quite sure it's not parser, since it work in simple tests with exact logic buffer
        let to_read = min(SEPARATE, min(buf.remaining(), remaining_data));
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
