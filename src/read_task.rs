use std::io;
use std::io::ErrorKind;

use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;

use crate::allocator;
use crate::parser::RpcParser;
use crate::rpc::{ConnectionContext, RpcCommand, ServerContext};

/// Reads RPC commands from a network connection, parses it,
/// and forwards them to a [`crate::vfs_task::VfsTask`].
pub struct ReadTask {
    readhalf: OwnedReadHalf,
    command_sender: UnboundedSender<RpcCommand>,
    server_context: ServerContext,
    connection_context: ConnectionContext,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn new(
        readhalf: OwnedReadHalf,
        command_sender: UnboundedSender<RpcCommand>,
        server_context: ServerContext,
        connection_context: ConnectionContext,
    ) -> Self {
        Self { readhalf, command_sender, server_context, connection_context }
    }

    /// Spawns a [`ReadTask`]  that reads commands from a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
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
            let request = match parser.parse_request().await {
                Ok(request) => request,
                Err(crate::rpc::Error::IO(err)) if err.kind() == ErrorKind::UnexpectedEof => {
                    return Ok(());
                }
                Err(error) => return Err(map_parser_error(error)),
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
