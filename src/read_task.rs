use std::io;
use std::io::ErrorKind;

use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;

use crate::allocator;
use crate::parser::{ParseFailure, RpcParser};
use crate::rpc::{CommandResult, ConnectionContext, RpcCommand, RpcReply, ServerContext};
use crate::serializer::serialize_reply;

/// Reads RPC commands from a network connection, parses it,
/// and forwards them to a [`crate::vfs_task::VfsTask`].
pub struct ReadTask {
    readhalf: OwnedReadHalf,
    command_sender: UnboundedSender<RpcCommand>,
    result_sender: UnboundedSender<CommandResult>,
    server_context: ServerContext,
    connection_context: ConnectionContext,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn new(
        readhalf: OwnedReadHalf,
        command_sender: UnboundedSender<RpcCommand>,
        result_sender: UnboundedSender<CommandResult>,
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
    pub fn spawn(self) {
        tokio::spawn(async move {
            if let Err(error) = self.run().await {
                eprintln!("read task error: {error}");
            } else {
                eprintln!("read task finished");
            }
        });
    }

    async fn run(self) -> io::Result<()> {
        let mut parser = RpcParser::with_capacity(
            self.readhalf,
            allocator::Impl::new(
                self.server_context.settings.allocator_buffer_size,
                self.server_context.settings.allocator_buffer_count,
            ),
            self.server_context.settings.read_buffer_size.get(),
        );

        loop {
            let request = match parser.parse_request_full().await {
                Ok(request) => request,
                Err(ParseFailure {
                    xid: None,
                    error: crate::rpc::Error::IO(err),
                }) if err.kind() == ErrorKind::UnexpectedEof => {
                    eprintln!("read task: client closed connection");
                    return Ok(());
                }
                Err(ParseFailure { xid: Some(xid), error }) => {
                    if is_expected_protocol_rejection(&error) {
                        eprintln!("read task rejected xid={xid}: {error:?}");
                    } else {
                        eprintln!("read task parse error xid={xid}: {error:?}");
                    }
                    match serialize_reply(xid, Err(error)).await {
                        Ok(payload) => {
                            if self.result_sender.send(Ok(RpcReply::new(xid, payload))).is_err() {
                                return Ok(());
                            }
                            continue;
                        }
                        Err(err) => return Err(err),
                    }
                }
                Err(ParseFailure { xid: None, error }) => {
                    if is_expected_protocol_rejection(&error) {
                        eprintln!("read task rejected request: {error:?}");
                    } else {
                        eprintln!("read task parse error: {error:?}");
                    }
                    return Err(map_parser_error(error));
                }
            };

            let command = request.with_connection(self.connection_context.clone());
            if self.command_sender.send(command).is_err() {
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
