use std::io::IoSlice;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::metric::io::IoTimer;

#[tokio::test]
async fn read_bytes_are_counted() {
    let (mut socket, stats) = IoTimer::new(&b"abc"[..]);

    let mut buf = [0; 8];
    let read = socket.read(&mut buf).await.unwrap();

    assert_eq!(&buf[..read], b"abc");
    assert_eq!(stats.take().1, 3);
}

#[tokio::test]
async fn take_returns_written_bytes_and_resets() {
    let (mut socket, stats) = IoTimer::new(Vec::new());

    socket.write_all(b"abc").await.unwrap();

    assert_eq!(stats.take().1, 3);
    assert_eq!(stats.take(), (Duration::ZERO, 0));
}

#[tokio::test]
async fn vectored_writes_are_forwarded() {
    let (mut socket, stats) = IoTimer::new(Vec::new());

    let written = socket.write_vectored(&[IoSlice::new(b"ab"), IoSlice::new(b"cd")]).await.unwrap();

    assert!(socket.is_write_vectored());
    assert_eq!(written, 4);
    assert_eq!(socket.inner, b"abcd");
    assert_eq!(stats.take().1, 4);
}
