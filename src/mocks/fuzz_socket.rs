use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

pub struct FuzzMockSocket {
    current: Vec<u8>,
    pos: usize,
    next: Option<Vec<u8>>,
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
            FuzzMockSocket { current: Vec::new(), pos: 0, next: None, recv },
            FuzzSocketHandler { sender },
        )
    }

    pub fn new_with_initial(initial: Vec<u8>) -> (Self, FuzzSocketHandler) {
        let (sender, recv) = mpsc::unbounded_channel();
        (
            FuzzMockSocket { current: initial, pos: 0, next: None, recv },
            FuzzSocketHandler { sender },
        )
    }

    fn poll_fill_next(&mut self) {
        if self.next.is_some() {
            return;
        }
        if let Ok(data) = self.recv.try_recv() {
            self.next = Some(data);
        }
    }

    fn ensure_current(&mut self) {
        if self.pos >= self.current.len() {
            if let Some(next) = self.next.take() {
                self.current = next;
                self.pos = 0;
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

        inner.poll_fill_next();
        inner.ensure_current();

        if inner.pos >= inner.current.len() {
            return Poll::Pending;
        }

        let mut read_any = false;

        while buf.remaining() > 0 {
            if inner.pos >= inner.current.len() {
                inner.ensure_current();
                if inner.pos >= inner.current.len() {
                    break;
                }
            }

            let remaining = inner.current.len() - inner.pos;
            if remaining == 0 {
                break;
            }

            let to_read = remaining.min(buf.remaining());
            let start = inner.pos;
            let end = start + to_read;

            buf.put_slice(&inner.current[start..end]);
            inner.pos += to_read;
            read_any = true;
        }

        if read_any {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}
