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
pub struct ReadTask;

impl ReadTask {
    /// Spawns an asynchronous task that reads RPC commands from a TCP stream.
    ///
    /// This task continuously reads data from the provided [`ReadHalf`] of a [`TcpStream`],
    /// parses it into [`RpcCommand`] objects, and sends them to a command processing queue
    /// via an unbounded channel. The task runs until the connection is closed or an
    /// unrecoverable error occurs.
    ///
    /// # Parameters
    /// - `readhalf`: The [`ReadHalf`] of a [`TcpStream`] used for reading incoming command data
    /// - `command_sender`: An [`UnboundedSender`] used to send parsed [`RpcCommand`] objects
    ///   to the command processing queue (typically handled by [`VfsTask`])
    ///
    /// # Behavior
    /// - **Successful Command Reading**: Parses command and forwards to processing queue
    /// - **Connection Closed Gracefully**: Returns `Ok(())` when EOF is received on empty buffer
    /// - **Connection Closed During Transmission**: Returns error when EOF is received mid-command
    /// - **Read Errors**: Returns the underlying I/O error for proper handling
    /// - **Queue Full/Closed**: Returns error when command cannot be enqueued
    ///
    /// # Error Handling
    /// - **`UnexpectedEof` with empty buffer**: Graceful connection closure (normal)
    /// - **`UnexpectedEof` with partial data**: Protocol violation (error)
    /// - **Other I/O errors**: Propagated for connection cleanup
    /// - **Channel send errors**: Indicates processing queue is unavailable
    pub fn spawn(mut readhalf: ReadHalf<TcpStream>, command_sender: UnboundedSender<RpcCommand>) {
        tokio::spawn(async move {
            loop {
                let mut command = RpcCommand { data: Vec::with_capacity(COMMAND_INIT_SIZE) };
                match command.read_command_from_socket(&mut readhalf).await {
                    Ok(()) => {
                        // here some processing - actually sending to processing rpc task
                        match command_sender.send(command) {
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
        });
    }
}
