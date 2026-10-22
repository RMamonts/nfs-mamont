//! Socket wrapper measuring the time spent in its I/O calls.

use std::io::{self, IoSlice};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[cfg(test)]
#[path = "io_tests.rs"]
mod io_tests;

/// Time spent in and bytes moved by the I/O calls of an [`IoTimer`] since the last [`IoStats::take`].
#[derive(Default)]
pub struct IoStats {
    nanos: AtomicU64,
    bytes: AtomicU64,
}

impl IoStats {
    /// Returns the accumulated I/O time and bytes, resetting both.
    pub fn take(&self) -> (Duration, u64) {
        let nanos = self.nanos.swap(0, Ordering::Relaxed);
        let bytes = self.bytes.swap(0, Ordering::Relaxed);
        (Duration::from_nanos(nanos), bytes)
    }

    fn record(&self, poll: impl FnOnce() -> Poll<io::Result<usize>>) -> Poll<io::Result<usize>> {
        let start = Instant::now();
        let result = poll();
        self.nanos.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
        if let Poll::Ready(Ok(n)) = result {
            self.bytes.fetch_add(n as u64, Ordering::Relaxed);
        }
        result
    }
}

/// Wraps a socket, adding the time spent in and bytes moved by its I/O calls to [`IoStats`].
pub struct IoTimer<S> {
    inner: S,
    stats: Arc<IoStats>,
}

impl<S> IoTimer<S> {
    pub fn new(inner: S) -> (Self, Arc<IoStats>) {
        let stats = Arc::new(IoStats::default());
        (Self { inner, stats: Arc::clone(&stats) }, stats)
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for IoTimer<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let filled = buf.filled().len();
        this.stats
            .record(|| {
                Pin::new(&mut this.inner)
                    .poll_read(cx, buf)
                    .map_ok(|()| buf.filled().len() - filled)
            })
            .map_ok(drop)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for IoTimer<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.stats.record(|| Pin::new(&mut this.inner).poll_write(cx, buf))
    }

    // Must be forwarded: the default implemenation writes only the first buffer
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.stats.record(|| Pin::new(&mut this.inner).poll_write_vectored(cx, bufs))
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
