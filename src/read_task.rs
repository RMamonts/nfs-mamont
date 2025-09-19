use crate::tcp::{RpcCommand, COMMAND_INIT_SIZE};
use crate::utils::error::io_other;
use std::io;
use tokio::io::ReadHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, trace};

pub struct ReadTask;

impl ReadTask {
    pub(crate) fn spawn(
        mut readhalf: ReadHalf<TcpStream>,
        command_sender: UnboundedSender<RpcCommand>,
    ) {
        tokio::spawn(async move {
            loop {
                let mut command = RpcCommand { data: Vec::with_capacity(COMMAND_INIT_SIZE) };
                match command.read_command_from_socket(&mut readhalf).await {
                    Ok(()) => {
                        //here some processing - actually sending to processing rpc task
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
