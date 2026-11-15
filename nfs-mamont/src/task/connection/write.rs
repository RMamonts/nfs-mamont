use std::io;
use std::time::Duration;

use tokio::net::tcp::OwnedWriteHalf;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::{error, warn};

use crate::allocator::Buffer;
use crate::connection_id::ConnectionId;
use crate::limits::budget::InFlightReply;
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;

/// Staged replies after which a batch is written even if more replies are queued.
const MAX_BATCH_BYTES: usize = 64 * 1024;

/// Writes [`super::super::global::vfs::VfsTask`] responses to a network connection.
pub struct WriteTask<B: Buffer> {
    writehalf: OwnedWriteHalf,
    result_receiver: async_channel::Receiver<InFlightReply<B>>,
    client_id: ConnectionId,
    /// How long one reply may wait for the socket, `None` for ever.
    write_timeout: Option<Duration>,
}

impl<B: Buffer> WriteTask<B> {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: async_channel::Receiver<InFlightReply<B>>,
        client_id: ConnectionId,
        write_timeout: Option<Duration>,
    ) -> Self {
        Self { writehalf, result_receiver, client_id, write_timeout }
    }

    /// Spawns a [`WriteTask`] that writes command results to a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) -> JoinHandle<()>
    where
        B: 'static,
    {
        tokio::spawn(async move { self.run().await })
    }

    async fn run(self) {
        let result_receiver = self.result_receiver;
        let mut serializer = serializer::server::serialize_struct::Serializer::new(self.writehalf);
        // Slots of the replies of the current batch, freed once the batch is written.
        let mut tokens = Vec::new();

        while let Ok(first) = result_receiver.recv().await {
            // Replies that are already queued are written together with the first one,
            // with a single syscall.
            let batch = async {
                let mut next = Some(first);
                while let Some(InFlightReply { reply, token }) = next.take() {
                    tokens.push(token);
                    // TODO: <https://github.com/RMamonts/nfs-mamont/issues/143>
                    // Use proper authentication verifier instead of None
                    let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };
                    if let Err(e) = serializer.stage_reply(reply, verifier).await {
                        if e.kind() != io::ErrorKind::InvalidInput
                            && e.kind() != io::ErrorKind::Unsupported
                        {
                            return Err(e);
                        }
                        error!(client=%self.client_id.into_inner(), error=%e, "write task: failed to serialize reply");
                    }
                    if serializer.staged_len() >= MAX_BATCH_BYTES {
                        break;
                    }
                    next = result_receiver.try_recv().ok();
                }
                serializer.send_staged().await
            };
            let written = match self.write_timeout {
                Some(limit) => timeout(limit, batch).await,
                None => Ok(batch.await),
            };
            let Ok(written) = written else {
                warn!(
                    client=%self.client_id.into_inner(),
                    timeout=?self.write_timeout,
                    "write task: the client does not take its replies, closing the connection"
                );
                return;
            };
            if let Err(e) = written {
                error!(client=%self.client_id.into_inner(), error=%e, "write task: failed to send replies");
                // TODO: Consider closing connection or continuing based on error type
                // For now, continue processing other replies
            }

            // Freeing the slots before the replies are written would let replies pile up.
            tokens.clear();
        }
    }
}
