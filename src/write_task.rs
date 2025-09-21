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
pub struct WriteTask;

impl WriteTask {
    /// Spawns an asynchronous task that handles writing command results to a TCP stream.
    ///
    /// This task continuously listens for [`CommandResult`] messages from a channel and
    /// writes appropriate responses to the provided TCP stream. The task runs until
    /// the result channel is closed or an error occurs during writing.
    ///
    /// # Parameters
    /// - `writehalf`: A [`WriteHalf`] of a [`TcpStream`] used for writing responses
    /// - `result_receiver`: An [`UnboundedReceiver`] channel that receives [`CommandResult`] messages
    ///
    /// # Behavior
    /// - For successful results with content: Writes the response buffer to the stream
    /// - For successful results without content: Silently acknowledges (no data sent)
    /// - For empty buffers: No action taken (buffer exists but contains no data)
    /// - For errors: Logs the error and terminates the task with the error
    ///
    /// # Errors
    /// Returns an error if writing to the TCP stream fails or if a command result
    /// contains an error that should terminate the connection.
    pub fn spawn(
        mut writehalf: WriteHalf<TcpStream>,
        mut result_receiver: UnboundedReceiver<CommandResult>,
    ) {
        tokio::spawn(async move {
            while let Some(result) = result_receiver.recv().await {
                match result {
                    Ok(Some(mut response_buffer)) if response_buffer.has_content() => {
                        if let Err(e) = response_buffer.write_fragment(&mut writehalf).await {
                            error!("Write error {:?}", e);
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
        });
    }
}
