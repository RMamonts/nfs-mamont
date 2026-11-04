use std::sync::Arc;
use std::time::Duration;

use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::OwnedSemaphorePermit;
use tokio::task::AbortHandle;
use tracing::{error, warn};

use crate::allocator::Buffer;
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::stats;
use crate::task::connection::limit::Limiter;
use crate::task::InFlightReply;

/// Writes [`super::super::global::vfs::VfsTask`] responses to a network connection.
pub struct WriteTask<B: Buffer> {
    writehalf: OwnedWriteHalf,
    result_receiver: async_channel::Receiver<InFlightReply<B>>,
    /// In-flight budget of the connection the written replies are returned to.
    limiter: Arc<Limiter>,
    /// The read task of the connection, stopped together with this task.
    read_task: AbortHandle,
    /// Slot of the connection in the global connection budget, if one is set.
    permit: Option<OwnedSemaphorePermit>,
    /// How long one reply may wait for the socket before the connection is closed.
    write_timeout: Option<Duration>,
}

/// Closes the budget and stops the read task when the write task stops for any reason,
/// including a panic, so that a read task parked on the budget does not wait forever
/// and a connection whose replies cannot be written does not linger.
struct CloseOnDrop<'a> {
    limiter: &'a Limiter,
    read_task: &'a AbortHandle,
}

impl Drop for CloseOnDrop<'_> {
    fn drop(&mut self) {
        self.limiter.close();
        self.read_task.abort();
    }
}

impl<B: Buffer> WriteTask<B> {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: async_channel::Receiver<InFlightReply<B>>,
        limiter: Arc<Limiter>,
        read_task: AbortHandle,
        permit: Option<OwnedSemaphorePermit>,
        write_timeout: Option<Duration>,
    ) -> Self {
        Self { writehalf, result_receiver, limiter, read_task, permit, write_timeout }
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
        let limiter = self.limiter;
        let _close_on_drop = CloseOnDrop { limiter: &limiter, read_task: &self.read_task };
        // Held until this task is gone, i.e. until the connection is closed.
        let _permit = self.permit;
        let mut serializer = serializer::server::serialize_struct::Serializer::new(self.writehalf);

        while let Ok(InFlightReply { reply, token }) = result_receiver.recv().await {
            // TODO: <https://github.com/RMamonts/nfs-mamont/issues/143>
            // Use proper authentication verifier instead of None
            let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };

            let written = match self.write_timeout {
                Some(timeout) => {
                    match tokio::time::timeout(timeout, serializer.form_reply(reply, verifier))
                        .await
                    {
                        Ok(written) => written,
                        Err(_) => {
                            // The socket did not take the reply in time: the client does
                            // not read. Dropping the writer closes the connection, the
                            // guard above stops the read task, and every token of the
                            // connection gives its resources back.
                            warn!(
                                timeout = ?timeout,
                                "write task: the client does not read its replies, closing \
                                 the connection"
                            );
                            stats::WRITE_TIMEOUTS
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            return;
                        }
                    }
                }
                None => serializer.form_reply(reply, verifier).await,
            };
            if let Err(e) = written {
                error!(error=%e, "write task: failed to serialize/send reply");
                // TODO: Consider closing connection or continuing based on error type
                // For now, continue processing other replies
            }

            // The request stays in flight until its reply bytes are handed to the socket:
            // freeing the slot earlier would let unwritten replies pile up again.
            limiter.release(token);
        }
    }
}
