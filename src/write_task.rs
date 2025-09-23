use std::io::Error;

use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, error};

use crate::tcp::CommandResult;

/// An asynchronous task responsible for writing [`VfsTask`] responses to a network connection.
pub struct WriteTask {
    writehalf: OwnedWriteHalf,
    result_receiver: UnboundedReceiver<CommandResult>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: UnboundedReceiver<CommandResult>,
    ) -> Self {
        Self { writehalf, result_receiver }
    }

    /// Spawns a [`WriteTask`]  that writes command results to a socket.
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

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
