use std::marker::PhantomData;

use tokio::net::tcp::OwnedWriteHalf;
use tracing::error;

use crate::allocator::Buffer;
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::task::ProcReply;

/// Maximum number of replies coalesced into a single socket write.
const MAX_BATCH_REPLIES: usize = 32;

/// Once the staging buffer reaches this size, the batch is flushed.
const FLUSH_THRESHOLD: usize = 64 * 1024;

/// Writes [`super::super::global::vfs::VfsPool`] responses to a network connection.
pub struct WriteTask<B: Buffer> {
    writehalf: OwnedWriteHalf,
    result_receiver: async_channel::Receiver<ProcReply<B>>,
    _phantom: PhantomData<B>,
}

impl<B: Buffer> WriteTask<B> {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: async_channel::Receiver<ProcReply<B>>,
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
        let result_receiver = self.result_receiver;
        let mut serializer =
            serializer::server::serialize_struct::Serializer::<B, _>::new(self.writehalf);

        while let Ok(reply) = result_receiver.recv().await {
            let mut next = Some(reply);
            let mut batched = 0;

            // Stage the received reply plus any replies already waiting in the
            // channel, then flush them all with a single socket write.
            while let Some(reply) = next.take() {
                // TODO: <https://github.com/RMamonts/nfs-mamont/issues/143>
                // Use proper authentication verifier instead of None
                let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };

                let staged = serializer.buffered_len();
                if let Err(e) = serializer.form_reply(reply, verifier).await {
                    error!(error=%e, "write task: failed to serialize/send reply");
                    // Drop the partially staged reply so the stream is not
                    // corrupted, but keep the complete replies staged before it.
                    // TODO: Consider closing connection or continuing based on error type
                    // For now, continue processing other replies
                    serializer.truncate_staged(staged);
                }
                batched += 1;

                if batched < MAX_BATCH_REPLIES && serializer.buffered_len() < FLUSH_THRESHOLD {
                    next = result_receiver.try_recv().ok();
                }
            }

            if let Err(e) = serializer.flush().await {
                error!(error=%e, "write task: failed to send replies");
                serializer.reset();
            }
        }
    }
}
