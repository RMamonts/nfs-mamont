use std::io::{self, IoSlice};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::AsyncWrite;

use super::Serializer;
use crate::allocator::Buffer;
use crate::consts::rpc::HEADER_MASK;
use crate::metric::Timer;
use crate::rpc::{AcceptStat, AuthFlavor, Error, OpaqueAuth};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::{read, NfsRes};

/// Payload buffer made of the given chunks.
struct Chunks(Vec<Vec<u8>>);

impl Buffer for Chunks {
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
        self.0.iter().map(Vec::as_slice)
    }

    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
        self.0.iter_mut().map(Vec::as_mut_slice)
    }

    fn len(&self) -> usize {
        self.0.iter().map(Vec::len).sum()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn empty() -> Self {
        Self(Vec::new())
    }
}

/// Writer accepting at most `max_write` bytes per call.
struct Socket {
    data: Vec<u8>,
    max_write: usize,
    writes: usize,
}

impl Socket {
    fn new(max_write: usize) -> Self {
        Self { data: Vec::new(), max_write, writes: 0 }
    }
}

impl AsyncWrite for Socket {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_write_vectored(cx, &[IoSlice::new(buf)])
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.writes += 1;
        let start = this.data.len();
        for buf in bufs {
            let left = this.max_write - (this.data.len() - start);
            this.data.extend_from_slice(&buf[..buf.len().min(left)]);
        }
        Poll::Ready(Ok(this.data.len() - start))
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

fn verifier() -> OpaqueAuth {
    OpaqueAuth { flavor: AuthFlavor::None, body: vec![] }
}

fn error_reply(xid: u32) -> ProcReply<Chunks> {
    ProcReply { xid, proc_result: Err(Error::ProcedureMismatch), timer: Timer::NONE }
}

fn read_reply(xid: u32, chunks: Vec<Vec<u8>>, count: u32) -> ProcReply<Chunks> {
    let head = read::SuccessHeader { file_attr: None, count, eof: false };
    let res = NfsRes::Read(Ok(read::Success { head, data: Chunks(chunks) }));
    ProcReply { xid, proc_result: Ok(ProcResult::Nfs3(Box::new(res))), timer: Timer::NONE }
}

fn record(words: &[u32], tail: &[u8]) -> Vec<u8> {
    let len = words.len() * 4 + tail.len();
    let mut record = ((HEADER_MASK | len) as u32).to_be_bytes().to_vec();
    words.iter().for_each(|word| record.extend_from_slice(&word.to_be_bytes()));
    record.extend_from_slice(tail);
    record
}

/// xid, REPLY, MSG_ACCEPTED, AUTH_NONE verifier, PROC_UNAVAIL
fn error_record(xid: u32) -> Vec<u8> {
    record(&[xid, 1, 0, 0, 0, AcceptStat::ProcUnavail as u32], &[])
}

/// xid, REPLY, MSG_ACCEPTED, AUTH_NONE verifier, SUCCESS,
/// NFS3_OK, no attributes, count, eof, opaque data
fn read_record(xid: u32, data: &[u8]) -> Vec<u8> {
    let count = data.len() as u32;
    let mut tail = data.to_vec();
    tail.resize(data.len().next_multiple_of(4), 0);
    record(&[xid, 1, 0, 0, 0, 0, 0, 0, count, 0, count], &tail)
}

async fn flush_one(replies: Vec<ProcReply<Chunks>>, max_write: usize) -> Socket {
    let mut serializer = Serializer::new(Socket::new(max_write));
    for reply in replies {
        serializer.form_reply(reply, verifier()).unwrap();
    }
    serializer.flush().await.unwrap();
    serializer.buffer.socket
}

#[tokio::test]
async fn batch_is_sent_with_one_write() {
    let replies = vec![
        error_reply(1),
        read_reply(2, vec![b"abcde".to_vec(), b"fgh".to_vec(), b"ijkl".to_vec()], 7),
        error_reply(3),
    ];

    let socket = flush_one(replies, usize::MAX).await;

    let expected = [error_record(1), read_record(2, b"abcdefg"), error_record(3)].concat();
    assert_eq!(socket.data, expected);
    assert_eq!(socket.writes, 1);
}

#[tokio::test]
async fn partial_writes_resume() {
    let replies = vec![
        error_reply(1),
        read_reply(2, vec![b"abcde".to_vec(), b"fgh".to_vec()], 6),
        error_reply(3),
    ];

    let socket = flush_one(replies, 3).await;

    let expected = [error_record(1), read_record(2, b"abcdef"), error_record(3)].concat();
    assert_eq!(socket.data, expected);
}

#[tokio::test]
async fn more_segments_than_iovecs() {
    let data: Vec<u8> = (0..99).collect();
    let chunks = data.iter().map(|&byte| vec![byte]).collect();

    let socket = flush_one(vec![read_reply(1, chunks, 99), error_reply(2)], usize::MAX).await;

    assert_eq!(socket.data, [read_record(1, &data), error_record(2)].concat());
    assert_eq!(socket.writes, 2);
}

#[tokio::test]
async fn failed_reply_is_not_staged() {
    let mut serializer = Serializer::new(Socket::new(usize::MAX));
    serializer.form_reply(error_reply(1), verifier()).unwrap();
    let staged = serializer.staged_len();

    let overflow = read_reply(2, vec![b"abc".to_vec()], 4);
    assert!(serializer.form_reply(overflow, verifier()).is_err());
    assert_eq!(serializer.staged_len(), staged);

    serializer.flush().await.unwrap();
    assert_eq!(serializer.buffer.socket.data, error_record(1));
    assert_eq!(serializer.staged_len(), 0);
}
