use std::io;

use tokio::io::ReadHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, trace};

use crate::rpc_command::RpcCommand;
use crate::utils::error::io_other;

/// Initial capacity of RpcCommand buffer
pub const COMMAND_INIT_SIZE: usize = 8192;

/// An asynchronous task responsible for reading RPC commands from a network connection.
///
/// The [`ReadTask`] handles the reading side of a client connection, continuously reading
/// data from the TCP stream, parsing it into [`RpcCommand`] objects, and forwarding them
/// to a command processing queue. It serves as the entry point for incoming client requests.
pub struct ReadTask {
    /// The read half of the TCP connection, used for receiving incoming command data
    readhalf: ReadHalf<TcpStream>,
    /// Channel sender for forwarding successfully parsed commands to the processing task
    command_sender: UnboundedSender<RpcCommand>,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn new(readhalf: ReadHalf<TcpStream>, command_sender: UnboundedSender<RpcCommand>) -> Self {
        Self { readhalf, command_sender }
    }

    /// Spawns a background task that processes RPC commands from a socket.
    ///
    /// This method moves ownership of the instance to a new Tokio task that will
    /// call method [`run()`](#method.run) to process RPC command
    ///
    /// # Panics
    ///
    /// This method does not panic. Any errors encountered during task execution
    /// are properly logged and the task exits cleanly.
    ///
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    /// Main function to process actual RPC-command
    ///
    /// This method runs an infinite loop that:
    /// 1. Creates a new command buffer
    /// 2. Reads a complete command from the socket
    /// 3. Sends the command to the processing task
    /// 4. Handles various error conditions appropriately
    ///
    /// # Exit Conditions
    ///
    /// Returns `Ok(())` in the following cases:
    /// - Socket closed gracefully before any data was received
    ///
    /// Returns `Err` in the following cases:
    /// - Socket closed during command transmission (`io_other("Early socket closing")`)
    /// - Command queue is disconnected (`io_other("Command queue error")`)
    /// - Any other I/O error during socket reading
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
