use std::mem;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::tcp::OwnedWriteHalf;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::{error, warn};

use crate::allocator::Buffer;
use crate::connection_id::ConnectionId;
use crate::limits::budget::InFlightReply;
use crate::metric::{reply_procedure, NfsMetrics, Stage, TaskKind, Timer};
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;

/// Writes [`super::super::global::vfs::VfsTask`] responses to a network connection.
pub struct WriteTask<B: Buffer> {
    writehalf: OwnedWriteHalf,
    result_receiver: async_channel::Receiver<InFlightReply<B>>,
    client_id: ConnectionId,
    /// How long one reply may wait for the socket, `None` for ever.
    write_timeout: Option<Duration>,
    metrics: Arc<NfsMetrics>,
}

impl<B: Buffer> WriteTask<B> {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: async_channel::Receiver<InFlightReply<B>>,
        client_id: ConnectionId,
        write_timeout: Option<Duration>,
        metrics: Arc<NfsMetrics>,
    ) -> Self {
        Self { writehalf, result_receiver, client_id, write_timeout, metrics }
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
        let metrics = Arc::clone(&self.metrics);
        tokio::spawn(metrics.instrument(TaskKind::Write, self.run()))
    }

    async fn run(self) {
        let result_receiver = self.result_receiver;
        let (writehalf, io) = self.metrics.io_timer(self.writehalf);
        let mut serializer = serializer::server::serialize_struct::Serializer::new(writehalf);

        while let Ok(InFlightReply { mut reply, token }) = result_receiver.recv().await {
            let mut timer = mem::replace(&mut reply.timer, Timer::NONE);
            self.metrics.lap(&mut timer, Stage::ReplyQueue);
            let procedure = reply_procedure(&reply);

            // TODO: <https://github.com/RMamonts/nfs-mamont/issues/143>
            // Use proper authentication verifier instead of None
            let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };

            let reply_future = serializer.form_reply(reply, verifier);
            let write = self.metrics.write_reply(&timer, procedure, &io, reply_future);
            let written = match self.write_timeout {
                Some(limit) => timeout(limit, write).await,
                None => Ok(write.await),
            };
            let Ok(written) = written else {
                warn!(
                    client=%self.client_id.into_inner(),
                    timeout=?self.write_timeout,
                    "write task: the client does not take its replies, closing the connection"
                );
                return;
            };
            match written {
                Ok(_) => {
                    // Reply successfully written to socket
                }
                Err(e) => {
                    error!(client=%self.client_id.into_inner(), error=%e, "write task: failed to serialize/send reply");
                    // TODO: Consider closing connection or continuing based on error type
                    // For now, continue processing other replies
                }
            };

            self.metrics.finish(timer);

            // Freeing the slot before the reply is written would let replies pile up.
            drop(token);
        }
    }
}
