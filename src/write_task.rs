use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::rpc::{CommandResult, ReplyPayload};

/// Writes [`crate::vfs_task::VfsTask`] responses to a network connection.
pub struct WriteTask {
    writehalf: OwnedWriteHalf,
    result_receiver: UnboundedReceiver<CommandResult>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: UnboundedReceiver<CommandResult>,
    ) -> Self {
        Self { writehalf, result_receiver }
    }

    /// Spawns a [`WriteTask`]  that writes command results to a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move {
            self.run().await;
            eprintln!("write task finished");
        });
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
                        eprintln!("write task: empty payload for xid={}", reply.xid);
                        continue;
                    }
                    if self.writehalf.write_all(&payload).await.is_err() {
                        eprintln!("write task: socket write failed for xid={}", reply.xid);
                        break;
                    }
                }
                ReplyPayload::Read { header, data, padding } => {
                    if self.writehalf.write_all(&header).await.is_err() {
                        eprintln!("write task: socket header write failed for xid={}", reply.xid);
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
                        eprintln!(
                            "write task: socket read-payload write failed for xid={}",
                            reply.xid
                        );
                        break;
                    }
                    if padding != 0 {
                        let zeros = [0u8; 4];
                        if self.writehalf.write_all(&zeros[..padding]).await.is_err() {
                            eprintln!(
                                "write task: socket padding write failed for xid={}",
                                reply.xid
                            );
                            break;
                        }
                    }
                }
            }
        }
    }
}
