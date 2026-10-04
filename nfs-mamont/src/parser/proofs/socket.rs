//! In-memory socket for parser harnesses.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};

/// Serves a fixed byte stream, then reports end of stream.
pub struct MockSocket {
    data: Vec<u8>,
    position: usize,
    segmented: bool,
}

impl MockSocket {
    /// Delivers `data` in reads of arbitrary non-zero length, as TCP may.
    pub fn segmented(data: &[u8]) -> Self {
        Self { data: data.to_vec(), position: 0, segmented: true }
    }

    /// Delivers as much of `data` as every read asks for.
    pub fn whole(data: &[u8]) -> Self {
        Self { data: data.to_vec(), position: 0, segmented: false }
    }
}

impl AsyncRead for MockSocket {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // A read into an empty buffer returns 0, which callers cannot tell
        // from end of stream.
        assert!(buf.remaining() > 0, "socket read into an empty buffer");

        let this = self.get_mut();
        let available = (this.data.len() - this.position).min(buf.remaining());
        let len = if this.segmented && available > 0 {
            kani::any_where(|len| (1..=available).contains(len))
        } else {
            available
        };
        buf.put_slice(&this.data[this.position..this.position + len]);
        this.position += len;
        Poll::Ready(Ok(()))
    }
}
