use std::mem;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::tcp::OwnedWriteHalf;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::{error, warn};

use crate::allocator::Buffer;
use crate::connection_id::ConnectionId;
use crate::metric::{CountedReceiver, NfsMetrics, Stage, TaskKind, Timer};
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::task::connection::limit::InFlightReply;
use crate::task::ProcResult;

/// Writes [`super::super::global::vfs::VfsTask`] responses to a network connection.
pub struct WriteTask<B: Buffer> {
    writehalf: OwnedWriteHalf,
    result_receiver: CountedReceiver<InFlightReply<B>>,
    metrics: Arc<NfsMetrics>,
    client_id: ConnectionId,
    /// How long one reply may wait for the socket, `None` for ever.
    write_timeout: Option<Duration>,
}

impl<B: Buffer> WriteTask<B> {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: CountedReceiver<InFlightReply<B>>,
        metrics: Arc<NfsMetrics>,
        client_id: ConnectionId,
        write_timeout: Option<Duration>,
    ) -> Self {
        Self { writehalf, result_receiver, metrics, client_id, write_timeout }
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
            let is_ok = reply.proc_result.is_ok();
            let procedure = match &reply.proc_result {
                Ok(ProcResult::Nfs3(res)) => Some(res.procedure()),
                _ => None,
            };

            // TODO: <https://github.com/RMamonts/nfs-mamont/issues/143>
            // Use proper authentication verifier instead of None
            let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };

            let write = serializer.form_reply(reply, verifier);
            let reply_future = self.metrics.write_reply(procedure, &io, write);

            let written = match self.write_timeout {
                Some(limit) => timeout(limit, reply_future).await,
                None => Ok(reply_future.await),
            };

            let Ok(written) = written else {
                warn!(
                    client=%self.client_id.into_inner(),
                    timeout=?self.write_timeout,
                    "write task: the client does not take its replies, closing the connection"
                );
                self.metrics.finish(timer, false);
                return;
            };
            let written = match written {
                Ok(_) => {
                    // Reply successfully written to socket
                    true
                }
                Err(e) => {
                    error!(client=%self.client_id.into_inner(), error=%e, "write task: failed to serialize/send reply");
                    // TODO: Consider closing connection or continuing based on error type
                    // For now, continue processing other replies
                    false
                }
            };

            self.metrics.finish(timer, is_ok && written);
            // Freeing the slot before the reply is written would let replies pile up.
            drop(token);
        }
    }
}
