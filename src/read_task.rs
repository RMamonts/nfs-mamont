use std::io;
use std::io::ErrorKind;

use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::allocator;
use crate::parser::{ParseFailure, RpcParser};
use crate::rpc::{
    rejected_request_span, ConnectionContext, ReplyEnvelope, RpcCommand, ServerContext,
};
use crate::serializer::serialize_reply;

/// Reads RPC commands from a network connection, parses it,
/// and forwards them to a [`crate::vfs_task::VfsTask`].
pub struct ReadTask {
    readhalf: OwnedReadHalf,
    command_sender: Sender<RpcCommand>,
    result_sender: Sender<ReplyEnvelope>,
    server_context: ServerContext,
    connection_context: ConnectionContext,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn new(
        readhalf: OwnedReadHalf,
        command_sender: Sender<RpcCommand>,
        result_sender: Sender<ReplyEnvelope>,
        server_context: ServerContext,
        connection_context: ConnectionContext,
    ) -> Self {
        Self { readhalf, command_sender, result_sender, server_context, connection_context }
    }

    /// Spawns a [`ReadTask`]  that reads commands from a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(error) = self.run().await {
                error!(error = %error, "read task failed");
            } else {
                info!("read task finished");
            }
        })
    }

    async fn run(self) -> io::Result<()> {
        let metrics = self.server_context.metrics();
        let mut parser = RpcParser::with_capacity(
            self.readhalf,
            allocator::Impl::new(
                self.server_context.settings().allocator_buffer_size(),
                self.server_context.settings().allocator_buffer_count(),
            ),
            self.server_context.settings().read_buffer_size().get(),
        );

        loop {
            let request = match parser.parse_request_full().await {
                Ok(request) => request,
                Err(ParseFailure { xid: None, error: crate::rpc::Error::IO(err) })
                    if err.kind() == ErrorKind::UnexpectedEof =>
                {
                    info!("read task detected client connection close");
                    return Ok(());
                }
                Err(ParseFailure { xid: Some(xid), error }) => {
                    metrics.record_request_rejected();
                    if is_expected_protocol_rejection(&error) {
                        warn!(xid, error = ?error, "read task rejected request");
                    } else {
                        debug!(xid, error = ?error, "read task parse error with reply");
                    }
                    match serialize_reply(xid, Err(error)).await {
                        Ok(reply) => {
                            let span = rejected_request_span(&self.connection_context, xid);
                            let received_at = std::time::Instant::now();
                            if self
                                .result_sender
                                .send(ReplyEnvelope::new(Ok(reply), span, received_at, None))
                                .await
                                .is_err()
                            {
                                return Ok(());
                            }
                            continue;
                        }
                        Err(err) => return Err(err),
                    }
                }
                Err(ParseFailure { xid: None, error }) => {
                    metrics.record_request_rejected();
                    if is_expected_protocol_rejection(&error) {
                        warn!(error = ?error, "read task rejected request without xid");
                    } else {
                        error!(error = ?error, "read task parse error without xid");
                    }
                    return Err(map_parser_error(error));
                }
            };

            let command = request.with_connection(self.connection_context.clone());
            let command_queue_depth = queue_depth(&self.command_sender).saturating_add(1);
            debug!(
                parent: &command.context.span,
                command_queue_depth,
                "parsed rpc request",
            );
            if self.command_sender.send(command).await.is_err() {
                return Ok(());
            }
            metrics.record_request_received(command_queue_depth);
        }
    }
}

fn map_parser_error(error: crate::rpc::Error) -> io::Error {
    match error {
        crate::rpc::Error::IO(err) => err,
        other => io::Error::new(ErrorKind::InvalidData, format!("{other:?}")),
    }
}

fn is_expected_protocol_rejection(error: &crate::rpc::Error) -> bool {
    matches!(
        error,
        crate::rpc::Error::RpcVersionMismatch(_)
            | crate::rpc::Error::ProgramMismatch
            | crate::rpc::Error::ProcedureMismatch
            | crate::rpc::Error::ProgramVersionMismatch(_)
            | crate::rpc::Error::AuthError(_)
            | crate::rpc::Error::MessageTypeMismatch
    )
}

fn queue_depth<T>(sender: &Sender<T>) -> usize {
    sender.max_capacity().saturating_sub(sender.capacity())
}
