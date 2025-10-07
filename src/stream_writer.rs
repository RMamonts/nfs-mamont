#![allow(dead_code)]
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

use crate::message_types::{EarlyReply, EarlyResult, ProcResult, Reply};

/// Writes [`vfs_task::VfsTask`] responses to a network connection.
pub struct StreamWriter {
    writehalf: OwnedWriteHalf,
    reply_recv: Receiver<Reply>,
    early_recv: Receiver<EarlyReply>,
}

impl StreamWriter {
    /// Creates new instance of [`StreamWriter`]
    pub fn spawn(
        writehalf: OwnedWriteHalf,
        reply_recv: Receiver<Reply>,
        early_recv: Receiver<EarlyReply>,
    ) -> JoinHandle<()> {
        tokio::spawn(Self { writehalf, reply_recv, early_recv }.run())
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                Some(early_reply) = self.early_recv.recv() => {
                    match early_reply.result {
                        EarlyResult::RPCError => {
                            todo!("Send error reply")
                        }
                        EarlyResult::Null => {
                            todo!("Send empty successful reply")
                        }
                    }
                },
                Some(reply) = self.reply_recv.recv() => {
                    match reply.result {
                        ProcResult::Error(_) => {
                            todo!("Send error message")
                        },
                        ProcResult::Ok(_) => {
                            todo!("Send successful reply")
                        }
                    }
                },
                else => {
                    todo!("MPSC channels closed");
                }
            }
        }
    }
}
