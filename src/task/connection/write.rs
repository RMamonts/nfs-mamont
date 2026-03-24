use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{error, info};

use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::task::ProcReply;

/// Writes [`crate::task::connection::vfs::VfsTask`] responses to a network connection.
pub struct WriteTask {
    writehalf: OwnedWriteHalf,
    result_receiver: UnboundedReceiver<ProcReply>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn new(writehalf: OwnedWriteHalf, result_receiver: UnboundedReceiver<ProcReply>) -> Self {
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
        let mut result_receiver = self.result_receiver;
        let buf_writer = tokio::io::BufWriter::with_capacity(128 * 1024, self.writehalf);
        let mut serializer = serializer::server::serialize_struct::Serializer::new(buf_writer);

        while let Some(reply) = result_receiver.recv().await {
            // Process the first received reply
            let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };
            info!(xid=%reply.xid, "write task: reply");
            if let Err(e) = serializer.form_reply(reply, verifier).await {
                error!(error=%e, "write task: failed to serialize/send reply");
            }

            // Drain any additionally available replies from the channel synchronously
            while let Ok(next_reply) = result_receiver.try_recv() {
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
