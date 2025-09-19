use crate::protocol::rpc::Context;
use crate::tcp::{
    process_rpc_command, CommandResult, ResponseBuffer, RpcCommand,
    DEFAULT_RESPONSE_BUFFER_CAPACITY,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error, trace};

pub struct VfsTask;

impl VfsTask {
    pub(crate) fn spawn(
        mut command_receiver: UnboundedReceiver<RpcCommand>,
        result_sender: UnboundedSender<CommandResult>,
        mut context: Context,
    ) {
        tokio::spawn(async move {
            // Create reusable buffer for responses
            let mut output_buffer = ResponseBuffer::with_capacity(DEFAULT_RESPONSE_BUFFER_CAPACITY);

            while let Some(command) = command_receiver.recv().await {
                trace!("Processing command from queue");

                // Clear buffer for reuse
                output_buffer.clear();
                // Call async processor
                let result =
                    match process_rpc_command(command.data, &mut output_buffer, &mut context).await
                    {
                        Ok(true) => {
                            // Processor indicated response needs to be sent
                            output_buffer.mark_has_content();
                            let buffer_to_send = std::mem::replace(
                                &mut output_buffer,
                                ResponseBuffer::with_capacity(DEFAULT_RESPONSE_BUFFER_CAPACITY),
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
    }
}
