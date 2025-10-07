#![allow(dead_code)]
use tokio::net::tcp::OwnedWriteHalf;
use tokio::task::JoinHandle;

use crate::message_types::{EarlyReplyRecv, EarlyResult, ProcResult, ReplyRecv};

/// Writes [`vfs_task::VfsTask`] responses to a network connection.
pub struct StreamWriter {
    writehalf: OwnedWriteHalf,
    reply_recv: ReplyRecv,
    early_recv: EarlyReplyRecv,
}

impl StreamWriter {
    /// Creates new instance of [`StreamWriter`]
    pub fn spawn(
        writehalf: OwnedWriteHalf,
        reply_recv: ReplyRecv,
        early_recv: EarlyReplyRecv,
    ) -> JoinHandle<()> {
        tokio::spawn(Self { writehalf, reply_recv, early_recv }.run())
    }

    async fn run(mut self) {
        loop {
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
                result = self.reply_recv.recv() => {
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
}
