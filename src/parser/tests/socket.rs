use std::cmp::min;
use std::io::Error;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub(super) struct MockSocket {
    data: Vec<u8>,
    position: usize,
}

impl MockSocket {
    pub fn new(buf: &[u8]) -> Self {
        MockSocket { data: buf.to_vec(), position: 0 }
    }
}

impl AsyncWrite for MockSocket {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        let inner = self.get_mut();
        inner.data.resize(buf.len(), 0);
        inner.data.copy_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncRead for MockSocket {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let data = self.get_mut();
        let position = data.position;
        let remaining = min(buf.remaining(), data.data.len());
        buf.put_slice(&data.data[position..remaining]);
        data.position += remaining;
        Poll::Ready(Ok(()))
    }
}
