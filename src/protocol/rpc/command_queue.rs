//! Command queue for ordered processing of RPC commands
//!
//! This module provides a command queue system that ensures RPC operations
//! are processed in the exact order they were received, preserving FIFO semantics
//! necessary for proper NFS protocol operation.

use std::io;
use tokio::sync::mpsc;
use tracing::{debug, error, trace};

use crate::protocol::rpc;
use crate::tcp::{CommandResult, ResponseBuffer, RpcCommand};

/// Type for asynchronous RPC command processor
pub type AsyncCommandProcessor = for<'a> fn(
    data: &[u8],
    output: &'a mut ResponseBuffer,
    context: rpc::Context,
) -> futures::future::BoxFuture<'a, io::Result<bool>>;

/// Queue for sequential processing of RPC commands
///
/// This structure manages an unbounded queue of RPC commands and processes
/// them sequentially to ensure proper operation order:
///
/// - Guaranteed FIFO command processing
/// - Asynchronous command submission
/// - Minimized data copying
/// - Separation of command submission from processing
#[derive(Debug, Clone)]
pub struct CommandQueue {
    /// Channel for sending commands
    command_sender: mpsc::UnboundedSender<RpcCommand>,
}

impl CommandQueue {
    /// Creates a new command queue with the given processor
    ///
    /// Initializes the command queue and starts a worker task that will
    /// process submitted commands in order. The processor function is
    /// responsible for handling each command and creating the result.
    ///
    /// # Arguments
    ///
    /// * `processor` - Asynchronous function for processing RPC commands
    /// * `result_sender` - Channel for sending processing results
    /// * `buffer_capacity` - Initial capacity for response buffers
    pub fn new(
        processor: AsyncCommandProcessor,
        result_sender: mpsc::UnboundedSender<CommandResult>,
        buffer_capacity: usize,
    ) -> Self {
        let (command_sender, mut command_receiver) = mpsc::unbounded_channel::<RpcCommand>();

        // Start worker task that processes commands in order
        tokio::spawn(async move {
            // Create reusable buffer for responses
            let mut output_buffer = ResponseBuffer::with_capacity(buffer_capacity);

            while let Some(command) = command_receiver.recv().await {
                trace!("Processing command from queue");

                // Clear buffer for reuse
                output_buffer.clear();

                // Call async processor
                let result =
                    match processor(&command.data, &mut output_buffer, command.context).await {
                        Ok(true) => {
                            // Processor indicated response needs to be sent
                            output_buffer.mark_has_content();
                            let buffer_to_send = std::mem::replace(
                                &mut output_buffer,
                                ResponseBuffer::with_capacity(buffer_capacity),
                            );
                            Ok(Some(buffer_to_send))
                        }
                        Ok(false) => {
                            // No response needed (e.g. retransmission)
                            Ok(None)
                        }
                        Err(e) => Err(e),
                    };

                // Send result
                if let Err(e) = result_sender.send(result) {
                    error!("Failed to send command processing result: {:?}", e);
                    break;
                }
            }
            debug!("Command queue handler finished");
        });

        Self { command_sender }
    }

    /// Submits a command to the queue for processing
    ///
    /// Commands are processed in the order they are submitted.
    /// This is an asynchronous operation that returns control immediately.
    ///
    /// # Arguments
    ///
    /// * `data` - RPC message data
    /// * `context` - Context for processing this command
    ///
    /// # Returns
    ///
    /// `true` if command was successfully submitted,
    /// `false` if submission failed (e.g. if queue was closed)
    pub fn submit_command(&self, command: RpcCommand) -> bool {
        self.command_sender.send(command).is_ok()
    }
}
