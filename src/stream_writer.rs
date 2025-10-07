use crate::message_types::{EarlyReply, EarlyResult, ProcResult, Reply};
use std::future::Future;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;

/// Writes [`vfs_task::VfsTask`] responses to a network connection.
pub struct StreamWriter {
    writehalf: OwnedWriteHalf,
    request_recv: UnboundedReceiver<Reply>,
    early_recv: UnboundedReceiver<EarlyReply>,
}

impl StreamWriter {
    /// Creates new instance of [`StreamWriter`]
    pub fn spawn(
        writehalf: OwnedWriteHalf,
        request_recv: UnboundedReceiver<Reply>,
        early_recv: UnboundedReceiver<EarlyReply>,
    ) -> JoinHandle<()> {
        tokio::spawn(Self { writehalf, request_recv, early_recv }.run())
    }

    async fn run(mut self) {
        tokio::select! {
            early_result = self.early_recv.recv() => {
                match early_result {
                    None => {
                        return;
                    }
                    Some(reply) => {
                        match reply.result {
                            EarlyResult::RPCError => {
                                todo!("Send error reply")
                            }
                            EarlyResult::Null => {
                                todo!("Send empty successful reply")
                            }
                        }
                    }
                }
            },
            result = self.request_recv.recv() => {
                match result {
                    None => {
                        return;
                    }
                    Some(reply) => {
                        match reply.result {
                            ProcResult::Error(_) => {
                                todo!("Send error message")
                            },
                            ProcResult::Ok(_) => {
                                todo!("Send successful reply")
                            }
                        }
                    }
                }
            }
        }
    }
}
