use crate::tcp::CommandResult;
use tokio::io::WriteHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, error};

pub struct WriteTask;

impl WriteTask {
    pub(crate) fn spawn(
        mut writehalf: WriteHalf<TcpStream>,
        mut result_receiver: UnboundedReceiver<CommandResult>,
    ) {
        //task to write to socket
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
