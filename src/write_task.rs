use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::rpc::CommandResult;

/// Writes [`crate::vfs_task::VfsTask`] responses to a network connection.
pub struct WriteTask {
    writehalf: OwnedWriteHalf,
    result_receiver: UnboundedReceiver<CommandResult>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn new(writehalf: OwnedWriteHalf, result_receiver: UnboundedReceiver<CommandResult>) -> Self {
        Self { writehalf, result_receiver }
    }

    /// Spawns a [`WriteTask`]  that writes command results to a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(mut self) {
        while let Some(result) = self.result_receiver.recv().await {
            let reply = match result {
                Ok(reply) => reply,
                Err(_) => continue,
            };

            if reply.payload.is_empty() {
                continue;
            }

            if self.writehalf.write_all(&reply.payload).await.is_err() {
                break;
            }
        }
    }
}
