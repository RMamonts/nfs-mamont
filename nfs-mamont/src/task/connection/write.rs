use std::mem;
use std::sync::Arc;

use async_channel::Receiver;
use tokio::net::tcp::OwnedWriteHalf;
use tracing::error;

use crate::allocator::Buffer;
use crate::metric::{NfsMetrics, Stage, TaskKind, Timer};
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::task::{ProcReply, ProcResult};

/// Max replies sent with one flush: bounds how long the first of them waits for the rest to be serialized.
const MAX_BATCH_REPLIES: usize = 32;
/// Max staged bytes per flush: bounds how long READ payloads are kept from the allocator.
/// Checked after staging a reply, so a larger reply is still sent.
const MAX_BATCH_BYTES: usize = 256 * 1024;

/// Writes [`super::super::global::vfs::VfsTask`] responses to a network connection.
pub struct WriteTask<B: Buffer> {
    writehalf: OwnedWriteHalf,
    result_receiver: Receiver<ProcReply<B>>,
    metrics: Arc<NfsMetrics>,
}

impl<B: Buffer> WriteTask<B> {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: Receiver<ProcReply<B>>,
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
        let mut timers = Vec::new();

        // Stage whatever replies are already queued and send them with one flush.
        // Never waits for more, so a lone reply is sent right away.
        while let Ok(first) = result_receiver.recv().await {
            let mut next = Some(first);
            // NFS replies staged and failed to serialize
            let (mut staged, mut failed) = (0, 0);
            // A measured reply that carries the flush metrics
            let mut measured = None;

            while let Some(mut reply) = next.take() {
                let mut timer = mem::replace(&mut reply.timer, Timer::NONE);
                self.metrics.lap(&mut timer, Stage::ReplyQueue);
                let procedure = match &reply.proc_result {
                    Ok(ProcResult::Nfs3(res)) => Some(res.procedure()),
                    _ => None,
                };

                // TODO: <https://github.com/RMamonts/nfs-mamont/issues/143>
                // Use proper authentication verifier instead of None
                let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };

                let result = self
                    .metrics
                    .serialize_reply(&timer, procedure, || serializer.form_reply(reply, verifier));
                match result {
                    Ok(()) if procedure.is_some() => {
                        staged += 1;
                        if timer.is_measured() {
                            measured = procedure;
                        }
                    }
                    Ok(()) => {}
                    Err(e) => {
                        error!(error=%e, "write task: failed to serialize reply");
                        failed += u64::from(procedure.is_some());
                    }
                }
                timers.push(timer);

                if timers.len() < MAX_BATCH_REPLIES && serializer.staged_len() < MAX_BATCH_BYTES {
                    next = result_receiver.try_recv().ok();
                }
            }

            let flush = serializer.flush();
            if let Err(e) = self.metrics.flush_replies(measured, &io, flush).await {
                error!(error=%e, "write task: failed to send replies");
                // TODO: Consider closing connection or continuing based on error type
                // For now, continue processing other replies
                (staged, failed) = (0, staged + failed);
            }
            self.metrics.count_replies(staged, failed);
            timers.drain(..).for_each(|timer| self.metrics.finish(timer));
        }
    }
}
