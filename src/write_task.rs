use std::io::{self, ErrorKind, IoSlice};
use std::sync::Arc;

use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::rpc::{ReplyEnvelope, ReplyPayload, ServerMetrics};

/// Writes [`crate::vfs_task::VfsTask`] responses to a network connection.
pub struct WriteTask {
    writehalf: OwnedWriteHalf,
    result_receiver: Receiver<ReplyEnvelope>,
    metrics: Arc<ServerMetrics>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: Receiver<ReplyEnvelope>,
        metrics: Arc<ServerMetrics>,
    ) -> Self {
        Self { writehalf, result_receiver, metrics }
    }

    /// Spawns a [`WriteTask`]  that writes command results to a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
            info!("write task finished");
        })
    }

    async fn run(mut self) {
        while let Some(envelope) = self.result_receiver.recv().await {
            let ReplyEnvelope { result, span, received_at, dispatched_at } = envelope;
            let reply = match result {
                Ok(reply) => reply,
                Err(error) => {
                    self.metrics.record_reply_failure();
                    warn!(parent: &span, error = %error, "write task dropped failed reply envelope");
                    continue;
                }
            };

            match reply.payload {
                ReplyPayload::Buffer(payload) => {
                    if payload.is_empty() {
                        self.metrics.record_reply_failure();
                        warn!(parent: &span, xid = reply.xid, "write task skipped empty payload");
                        continue;
                    }
                    if self.writehalf.write_all(&payload).await.is_err() {
                        self.metrics.record_reply_failure();
                        warn!(parent: &span, xid = reply.xid, "write task socket write failed");
                        break;
                    }
                }
                ReplyPayload::Read { header, data, padding } => {
                    let mut chunks = data.iter();
                    if let Some(first_chunk) = chunks.next() {
                        if write_header_and_first_chunk(&mut self.writehalf, &header, first_chunk)
                            .await
                            .is_err()
                        {
                            self.metrics.record_reply_failure();
                            warn!(parent: &span, xid = reply.xid, "write task vectored read write failed");
                            break;
                        }

                        let mut failed = false;
                        for chunk in chunks {
                            if self.writehalf.write_all(chunk).await.is_err() {
                                failed = true;
                                break;
                            }
                        }
                        if failed {
                            self.metrics.record_reply_failure();
                            warn!(parent: &span, xid = reply.xid, "write task read-payload write failed");
                            break;
                        }
                    } else if self.writehalf.write_all(&header).await.is_err() {
                        self.metrics.record_reply_failure();
                        warn!(parent: &span, xid = reply.xid, "write task socket header write failed");
                        break;
                    }

                    if padding != 0 {
                        let zeros = [0u8; 4];
                        if self.writehalf.write_all(&zeros[..padding]).await.is_err() {
                            self.metrics.record_reply_failure();
                            warn!(parent: &span, xid = reply.xid, "write task socket padding write failed");
                            break;
                        }
                    }
                }
            }

            let total_latency_micros = received_at.elapsed().as_micros() as u64;
            let dispatch_to_write_micros = dispatched_at
                .map(|instant| instant.elapsed().as_micros() as u64)
                .unwrap_or_default();
            self.metrics.record_reply_sent(total_latency_micros, dispatch_to_write_micros);

            debug!(
                parent: &span,
                xid = reply.xid,
                total_latency_micros,
                dispatch_to_write_micros,
                "write task sent reply",
            );
        }
    }
}

async fn write_header_and_first_chunk<W: AsyncWrite + Unpin>(
    writer: &mut W,
    header: &[u8],
    first_chunk: &[u8],
) -> io::Result<()> {
    let mut header_offset = 0;
    let mut chunk_offset = 0;

    while header_offset < header.len() || chunk_offset < first_chunk.len() {
        let written = match (header_offset < header.len(), chunk_offset < first_chunk.len()) {
            (true, true) => {
                let buffers = [
                    IoSlice::new(&header[header_offset..]),
                    IoSlice::new(&first_chunk[chunk_offset..]),
                ];
                writer.write_vectored(&buffers).await?
            }
            (true, false) => writer.write(&header[header_offset..]).await?,
            (false, true) => writer.write(&first_chunk[chunk_offset..]).await?,
            (false, false) => break,
        };

        if written == 0 {
            return Err(io::Error::new(
                ErrorKind::WriteZero,
                "failed to write header and first read chunk",
            ));
        }

        let header_remaining = header.len().saturating_sub(header_offset);
        let header_written = written.min(header_remaining);
        header_offset += header_written;
        chunk_offset += written.saturating_sub(header_written);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::io::IoSlice;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use tokio::io::AsyncWrite;

    use super::write_header_and_first_chunk;

    struct PartialWriter {
        buf: Vec<u8>,
        max_write: usize,
        vectored_calls: usize,
    }

    impl PartialWriter {
        fn new(max_write: usize) -> Self {
            Self { buf: Vec::new(), max_write, vectored_calls: 0 }
        }
    }

    impl AsyncWrite for PartialWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let count = buf.len().min(self.max_write);
            self.buf.extend_from_slice(&buf[..count]);
            Poll::Ready(Ok(count))
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<io::Result<usize>> {
            self.vectored_calls += 1;
            let mut remaining = self.max_write;
            let mut written = 0;

            for buf in bufs {
                if remaining == 0 {
                    break;
                }
                let count = buf.len().min(remaining);
                self.buf.extend_from_slice(&buf[..count]);
                written += count;
                remaining -= count;
            }

            Poll::Ready(Ok(written))
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

    #[tokio::test]
    async fn write_header_and_first_chunk_handles_partial_vectored_writes() {
        let mut writer = PartialWriter::new(3);

        write_header_and_first_chunk(&mut writer, &[1, 2, 3, 4], &[5, 6, 7, 8]).await.unwrap();

        assert_eq!(writer.buf, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(writer.vectored_calls > 0);
    }
}
