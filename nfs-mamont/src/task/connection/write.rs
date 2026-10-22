use std::mem;
use std::sync::Arc;

use tokio::net::tcp::OwnedWriteHalf;
use tracing::error;

use crate::allocator::Buffer;
use crate::metric::{CountedReceiver, NfsMetrics, Stage, TaskKind, Timer};
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::task::{ProcReply, ProcResult};

/// Writes [`super::super::global::vfs::VfsTask`] responses to a network connection.
pub struct WriteTask<B: Buffer> {
    writehalf: OwnedWriteHalf,
    result_receiver: CountedReceiver<ProcReply<B>>,
    metrics: Arc<NfsMetrics>,
}

impl<B: Buffer> WriteTask<B> {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: CountedReceiver<ProcReply<B>>,
        metrics: Arc<NfsMetrics>,
    ) -> Self {
        Self { writehalf, result_receiver, metrics }
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
        let metrics = Arc::clone(&self.metrics);
        tokio::spawn(metrics.instrument(TaskKind::Write, self.run()));
    }

    async fn run(self) {
        let result_receiver = self.result_receiver;
        let (writehalf, io) = self.metrics.io_timer(self.writehalf);
        let mut serializer = serializer::server::serialize_struct::Serializer::new(writehalf);

        while let Ok(mut reply) = result_receiver.recv().await {
            let mut timer = mem::replace(&mut reply.timer, Timer::NONE);
            self.metrics.lap(&mut timer, Stage::ReplyQueue);
            let is_ok = timer.is_measured() // const false if metrics are disabled
                && match &reply.proc_result {
                    Ok(ProcResult::Nfs3(res)) => res.error_from_response().is_none(),
                    Ok(_) => true,
                    Err(_) => false,
                };
            let procedure = match &reply.proc_result {
                Ok(ProcResult::Nfs3(res)) => Some(res.procedure()),
                _ => None,
            };

            // TODO: <https://github.com/RMamonts/nfs-mamont/issues/143>
            // Use proper authentication verifier instead of None
            let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };

            let reply = serializer.form_reply(reply, verifier);
            let written = match self.metrics.write_reply(procedure, &io, reply).await {
                Ok(_) => {
                    // Reply successfully written to socket
                    true
                }
                Err(e) => {
                    error!(error=%e, "write task: failed to serialize/send reply");
                    // TODO: Consider closing connection or continuing based on error type
                    // For now, continue processing other replies
                    false
                }
            };

            self.metrics.lap(&mut timer, Stage::Write);
            self.metrics.finish(timer, is_ok && written);
        }
    }
}
