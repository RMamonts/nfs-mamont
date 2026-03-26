use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::Receiver;
use tracing::{error, info};

use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::task::ProcReply;

/// Writes [`crate::task::connection::vfs::VfsTask`] responses to a network connection.
pub struct WriteTask {
    writehalf: OwnedWriteHalf,
    result_receiver: Receiver<ProcReply>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn new(writehalf: OwnedWriteHalf, result_receiver: Receiver<ProcReply>) -> Self {
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

    async fn run(self) {
        const MAX_BATCH_REPLIES: usize = 32;
        const BATCH_WINDOW: std::time::Duration = std::time::Duration::from_millis(1);

        let mut result_receiver = self.result_receiver;
        let mut serializer = serializer::server::serialize_struct::Serializer::new(self.writehalf);

        'outer: while let Some(reply) = result_receiver.recv().await {
            // Process the first received reply
            let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };
            info!(xid=%reply.xid, "write task: reply");
            if let Err(e) = serializer.form_reply(reply, verifier).await {
                error!(error=%e, "write task: failed to serialize/send reply");
            }

            // Batch a small time window to improve throughput on non-local networks.
            for _ in 0..MAX_BATCH_REPLIES.saturating_sub(1) {
                let next_reply =
                    match tokio::time::timeout(BATCH_WINDOW, result_receiver.recv()).await {
                        Ok(Some(next_reply)) => next_reply,
                        Ok(None) => break 'outer,
                        Err(_) => break,
                    };
                let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };
                info!(xid=%next_reply.xid, "write task: reply (batched)");
                if let Err(e) = serializer.form_reply(next_reply, verifier).await {
                    error!(error=%e, "write task: failed to serialize/send reply");
                }
            }

            // After draining all ready replies, flush the buffered writer to the socket.
            if let Err(e) = serializer.flush().await {
                error!(error=%e, "write task: failed to flush buffered replies");
            }
        }
    }
}
