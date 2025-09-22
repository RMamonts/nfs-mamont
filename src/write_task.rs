use std::io::Error;
use tokio::io::WriteHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, error};

use crate::tcp::CommandResult;

/// An asynchronous task responsible for writing data to a network connection.
///
/// The `WriteTask` handles the writing side of a client connection, processing
/// command results and writing appropriate responses to the TCP stream. It runs
/// as a background task that consumes results from a channel and writes them
/// to the network connection.
pub struct WriteTask {
    /// The write half of the TCP connection for sending response data
    writehalf: WriteHalf<TcpStream>,
    /// Channel receiver for receiving command processing results to send back to client
    result_receiver: UnboundedReceiver<CommandResult>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: WriteHalf<TcpStream>,
        result_receiver: UnboundedReceiver<CommandResult>,
    ) -> Self {
        Self { writehalf, result_receiver }
    }
    //// Spawns a background task that writes command results to a socket.
    ///
    /// This method moves ownership of the instance to a new Tokio task that will
    /// call method [`run()`](#method.run) to process and write command results.
    ///
    /// # Panics
    ///
    /// This method does not panic. Any errors encountered during task execution
    /// are properly logged and the task exits cleanly.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    /// Main function to process and write command results to the network connection.
    ///
    /// This method runs a loop that:
    /// 1. Receives command results from the result channel
    /// 2. Writes response data to the TCP stream when appropriate
    /// 3. Handles different result types efficiently
    async fn run(mut self) -> Result<(), Error> {
        while let Some(result) = self.result_receiver.recv().await {
            match result {
                Ok(Some(mut response_buffer)) if response_buffer.has_content() => {
                    if let Err(e) = response_buffer.write_fragment(&mut self.writehalf).await {
                        error!("Write error {:?}", e);
                        return Err(e);
                    }
                }
                Ok(None) => {
                    // No response needed, so nothing to send
                }
                Ok(Some(_)) => {
                    // Buffer exists but contains no data to send
                }
                Err(e) => {
                    debug!("Message handling closed : {:?}", e);
                    return Err(e);
                }
            }
        }
        debug!("Command result handler finished");
        Ok(())
    }
}
