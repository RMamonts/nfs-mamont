use std::sync::Arc;

use tokio::net::tcp::OwnedWriteHalf;
use tracing::error;

use crate::allocator::Buffer;
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::task::connection::limit::Limiter;
use crate::task::InFlightReply;

/// Writes [`super::super::global::vfs::VfsTask`] responses to a network connection.
pub struct WriteTask<B: Buffer> {
    writehalf: OwnedWriteHalf,
    result_receiver: async_channel::Receiver<InFlightReply<B>>,
    /// In-flight budget of the connection the written replies are returned to.
    limiter: Arc<Limiter>,
}

/// Closes the budget when the write task stops for any reason, including a panic, so
/// that a read task parked on it does not wait forever.
struct CloseOnDrop<'a>(&'a Limiter);

impl Drop for CloseOnDrop<'_> {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl<B: Buffer> WriteTask<B> {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: async_channel::Receiver<InFlightReply<B>>,
        limiter: Arc<Limiter>,
    ) -> Self {
        Self { writehalf, result_receiver, limiter }
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
        let Self { writehalf, result_receiver, limiter } = self;
        let _close_on_drop = CloseOnDrop(&limiter);
        let mut serializer = serializer::server::serialize_struct::Serializer::new(writehalf);

        while let Ok(InFlightReply { reply, token }) = result_receiver.recv().await {
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

            // The request stays in flight until its reply bytes are handed to the socket:
            // freeing the slot earlier would let unwritten replies pile up again.
            limiter.release(token);
        }
    }
}
