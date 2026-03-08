use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::rpc::{CommandResult, ReplyPayload};

/// Writes [`crate::vfs_task::VfsTask`] responses to a network connection.
pub struct WriteTask {
    writehalf: OwnedWriteHalf,
    result_receiver: Receiver<CommandResult>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn new(writehalf: OwnedWriteHalf, result_receiver: Receiver<CommandResult>) -> Self {
        Self { writehalf, result_receiver }
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
        while let Some(result) = self.result_receiver.recv().await {
            let reply = match result {
                Ok(reply) => reply,
                Err(_) => continue,
            };

            match reply.payload {
                ReplyPayload::Buffer(payload) => {
                    if payload.is_empty() {
                        warn!(xid = reply.xid, "write task skipped empty payload");
                        continue;
                    }
                    if self.writehalf.write_all(&payload).await.is_err() {
                        warn!(xid = reply.xid, "write task socket write failed");
                        break;
                    }
                }
                ReplyPayload::Read { header, data, padding } => {
                    if self.writehalf.write_all(&header).await.is_err() {
                        warn!(xid = reply.xid, "write task socket header write failed");
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
                        warn!(xid = reply.xid, "write task read-payload write failed");
                        break;
                    }
                    if padding != 0 {
                        let zeros = [0u8; 4];
                        if self.writehalf.write_all(&zeros[..padding]).await.is_err() {
                            warn!(xid = reply.xid, "write task socket padding write failed");
                            break;
                        }
                    }
                }
            }

            debug!(xid = reply.xid, "write task sent reply");
        }
    }
}
