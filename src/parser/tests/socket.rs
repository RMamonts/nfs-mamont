use std::cmp::min;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};

const SEPARATE: usize = 15;

pub(super) struct MockSocket {
    data: Vec<u8>,
    position: usize,
}

impl MockSocket {
    pub fn new(buf: &[u8]) -> Self {
        MockSocket { data: buf.to_vec(), position: 0 }
    }
}

impl AsyncRead for MockSocket {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let inner = self.get_mut();
        if inner.position >= inner.data.len() {
            return Poll::Ready(Ok(()));
        }
        let remaining_data = inner.data.len() - inner.position;

        let to_read = min(SEPARATE, min(buf.remaining(), remaining_data));

        if to_read > 0 {
            let start = inner.position;
            let end = inner.position + to_read;
            buf.put_slice(&inner.data[start..end]);

            inner.position += to_read;
        }

        Poll::Ready(Ok(()))
    }
}
