use std::io;
use std::io::{Cursor, Error};
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, error};

use crate::tcp::CommandResult;
use crate::xdr::rpc::{accepted_reply, opaque_auth, reply_body, rpc_body, rpc_msg};
use crate::xdr::{ProtocolErrors, Serialize};

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
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(mut self) -> Result<(), Error> {
        while let Some((xid, result)) = self.result_receiver.recv().await {
            match result {
                Ok(mut response_buffer) => {
                    if let Err(e) = response_buffer.write_fragment(&mut self.writehalf).await {
                        error!("Write error {:?}", e);
                        return Err(e);
                    }
                }
                Err(e) => {
                    return match self.error_replying(xid, e).await {
                        Ok(_) => {
                            debug!("Replying successfully to client with error");
                            Ok(())
                        }
                        Err(e) => {
                            error!("Failed to send error reply to client: {:?}", e);
                            Err(e)
                        }
                    }
                }
            }
        }
        debug!("Command result handler finished");
        Ok(())
    }

    async fn error_replying(&mut self, xid: u32, error: ProtocolErrors) -> io::Result<()> {
        debug!("Replying with protocol error : {:?}", error);
        let mut buf = Cursor::new(vec![0_u8; 450]);
        match error {
            ProtocolErrors::RpcRejected(e) => {
                rpc_msg { xid, body: rpc_body::REPLY(reply_body::MSG_DENIED(e)) }
                    .serialize(&mut buf)?;
                self.writehalf.write_all(&buf.into_inner()).await
            }
            ProtocolErrors::RpcAccepted(e) => {
                reply_body::MSG_ACCEPTED(accepted_reply {
                    verf: opaque_auth::default(),
                    reply_data: e,
                })
                .serialize(&mut buf)?;
                self.writehalf.write_all(&buf.into_inner()).await
            }
            _ => {
                todo!()
            }
        }
    }
}
