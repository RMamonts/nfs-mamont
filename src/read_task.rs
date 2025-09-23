use std::io;

use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, trace};

use crate::rpc_command::RpcCommand;
use crate::utils::error::io_other;

/// Initial capacity of RpcCommand buffer
pub const COMMAND_INIT_SIZE: usize = 8192;

/// An asynchronous task responsible for reading RPC commands from a network connection,
/// parsing it into [`RpcCommand`] objects, and forwarding them to a [`VfsTask`].
pub struct ReadTask {
    readhalf: OwnedReadHalf,
    command_sender: UnboundedSender<RpcCommand>,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn new(readhalf: OwnedReadHalf, command_sender: UnboundedSender<RpcCommand>) -> Self {
        Self { readhalf, command_sender }
    }

    /// Spawns a [`ReadTask`]  that reads commands from a socket.
    ///
    /// # Panics
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(mut self) -> io::Result<()> {
        loop {
            let mut command = RpcCommand { data: Vec::with_capacity(COMMAND_INIT_SIZE) };
            match command.read_command_from_socket(&mut self.readhalf).await {
                Ok(()) => {
                    // here some processing - actually sending to processing rpc task
                    match self.command_sender.send(command) {
                        Ok(_) => continue,
                        Err(_) => {
                            error!("Failed to submit command to queue");
                            return io_other("Command queue error");
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    return if command.data.is_empty() {
                        trace!("Connection closed before receiving any data");
                        Ok(())
                    } else {
                        error!("Connection closed during command transmission");
                        io_other("Early socket closing")
                    }
                }
                Err(e) => {
                    error!("Message loop broken due to {:?}", e);
                    return Err(e);
                }
            }
        }
    }
}
