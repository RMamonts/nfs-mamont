use std::io;
use std::io::ErrorKind;

use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::allocator;
use crate::parser::{ParseFailure, RpcParser};
use crate::rpc::{CommandResult, ConnectionContext, RpcCommand, ServerContext};
use crate::serializer::serialize_reply;

/// Reads RPC commands from a network connection, parses it,
/// and forwards them to a [`crate::vfs_task::VfsTask`].
pub struct ReadTask {
    readhalf: OwnedReadHalf,
    command_sender: Sender<RpcCommand>,
    result_sender: Sender<CommandResult>,
    server_context: ServerContext,
    connection_context: ConnectionContext,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn new(
        readhalf: OwnedReadHalf,
        command_sender: Sender<RpcCommand>,
        result_sender: Sender<CommandResult>,
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
                    if is_expected_protocol_rejection(&error) {
                        warn!(xid, error = ?error, "read task rejected request");
                    } else {
                        debug!(xid, error = ?error, "read task parse error with reply");
                    }
                    match serialize_reply(xid, Err(error)).await {
                        Ok(reply) => {
                            if self.result_sender.send(Ok(reply)).await.is_err() {
                                return Ok(());
                            }
                            continue;
                        }
                        Err(err) => return Err(err),
                    }
                }
                Err(ParseFailure { xid: None, error }) => {
                    if is_expected_protocol_rejection(&error) {
                        warn!(error = ?error, "read task rejected request without xid");
                    } else {
                        error!(error = ?error, "read task parse error without xid");
                    }
                    return Err(map_parser_error(error));
                }
            };

            let command = request.with_connection(self.connection_context.clone());
            if self.command_sender.send(command).await.is_err() {
                return Ok(());
            }
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
