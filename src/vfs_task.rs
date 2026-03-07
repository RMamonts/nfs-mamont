use std::io;
use std::io::ErrorKind;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::rpc::{CommandResult, RpcCommand, ServerContext};

/// Process RPC commands, sends operation results to [`crate::write_task::WriteTask`].
pub struct VfsTask {
    command_receiver: UnboundedReceiver<RpcCommand>,
    result_sender: UnboundedSender<CommandResult>,
    server_context: ServerContext,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn new(
        command_receiver: UnboundedReceiver<RpcCommand>,
        result_sender: UnboundedSender<CommandResult>,
        server_context: ServerContext,
    ) -> Self {
        Self { command_receiver, result_sender, server_context }
    }

    /// Spawns a [`VfsTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(mut self) {
        while let Some(command) = self.command_receiver.recv().await {
            let _ = &self.server_context;
            let _ = &command.arguments;

            let result = Err(io::Error::new(
                ErrorKind::Unsupported,
                format!(
                    "RPC dispatch is not implemented yet for program={}, version={}, procedure={}",
                    command.context.header.program,
                    command.context.header.version,
                    command.context.header.procedure,
                ),
            ));

            if self.result_sender.send(result).is_err() {
                break;
            }
        }
    }
}
