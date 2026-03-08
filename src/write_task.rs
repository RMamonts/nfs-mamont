use std::sync::Arc;

use tokio::io::AsyncWriteExt;
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
                    if self.writehalf.write_all(&header).await.is_err() {
                        self.metrics.record_reply_failure();
                        warn!(parent: &span, xid = reply.xid, "write task socket header write failed");
                        break;
                    }
                    let mut failed = false;
                    for chunk in data.iter() {
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
