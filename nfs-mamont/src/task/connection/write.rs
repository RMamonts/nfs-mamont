use std::marker::PhantomData;

use tokio::net::tcp::OwnedWriteHalf;
use tracing::error;

use crate::allocator::Buffer;
use crate::nlm::cookie::Cookie;
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::task::{ProcCall, ProcReply};

/// Writes [`super::super::global::vfs::VfsPool`] responses to a network connection.
pub struct WriteTask<B: Buffer> {
    writehalf: OwnedWriteHalf,
    result_receiver: async_channel::Receiver<ProcReply<B>>,
    /// The channel for sending the callback.
    message_receiver: async_channel::Receiver<ProcCall>,
    _phantom: PhantomData<B>,
}

impl<B: Buffer> WriteTask<B> {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: async_channel::Receiver<ProcReply<B>>,
        message_receiver: async_channel::Receiver<ProcCall>,
    ) -> Self {
        Self { writehalf, result_receiver, message_receiver, _phantom: PhantomData }
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
        let message_receiver = self.message_receiver;
        let mut serializer =
            serializer::server::serialize_struct::Serializer::<B, _>::new(self.writehalf);

        loop {
            tokio::select! {
                reply = result_receiver.recv() => {
                    let Ok(reply) = reply else { break }; // client gone
                    // TODO: <https://github.com/RMamonts/nfs-mamont/issues/143>
                    // Use proper authentication verifier instead of None
                    let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };

                    if let Err(e) = serializer.form_reply(reply, verifier).await {
                        error!(error=%e, "write task: failed to serialize/send reply");
                        // TODO: Consider closing connection or continuing based on error type
                        // For now, continue processing other replies
                    }
                }
                call = message_receiver.recv() => {
                    let Ok(call) = call else { continue }; // callbacks done; keep serving replies
                    let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };
                    let credential = verifier.clone();

                    if let Err(e) = serializer.form_call(call, credential, verifier).await {
                        error!(error=%e, "write task: failed to serialize/send reply");
                        // TODO: Consider closing connection or continuing based on error type
                        // For now, continue processing other replies
                    }
                }
            }
        }
    }
}
