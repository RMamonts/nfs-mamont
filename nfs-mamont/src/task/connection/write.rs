use std::marker::PhantomData;

use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::error;

use crate::allocator::Buffer;
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::task::ProcReply;

/// Writes [`super::super::global::vfs::VfsPool`] responses to a network connection.
pub struct WriteTask<B: Buffer> {
    writehalf: OwnedWriteHalf,
    result_receiver: UnboundedReceiver<ProcReply<B>>,
    _phantom: PhantomData<B>,
}

impl<B: Buffer> WriteTask<B> {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: UnboundedReceiver<ProcReply<B>>,
    ) -> Self {
        Self { writehalf, result_receiver, _phantom: PhantomData }
    }

    /// Spawns a [`WriteTask`] that writes command results to a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self)
    where
        B: 'static,
    {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let mut result_receiver = self.result_receiver;
        let mut serializer =
            serializer::server::serialize_struct::Serializer::<B, _>::new(self.writehalf);

        while let Some(reply) = result_receiver.recv().await {
            // TODO: <https://github.com/RMamonts/nfs-mamont/issues/143>
            // Use proper authentication verifier instead of None
            let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };

            match serializer.form_reply(reply, verifier).await {
                Ok(_) => {
                    // Reply successfully written to socket
                }
                Err(e) => {
                    error!(error=%e, "write task: failed to serialize/send reply");
                    // TODO: Consider closing connection or continuing based on error type
                    // For now, continue processing other replies
                }
            };
        }
    }
}
