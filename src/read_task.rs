#![allow(dead_code)]
use tokio::net::tcp::OwnedReadHalf;
use tokio::task::JoinHandle;

use crate::message_types::{EarlyReplySender, ProcSender};

/// Reads RPC commands from a network connection, parses it,
/// and forwards them to a [`crate::vfs_task::VfsTask`].
pub struct ReadTask {
    readhalf: OwnedReadHalf,
    proc_send: ProcSender,
    early_send: EarlyReplySender,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn spawn(
        readhalf: OwnedReadHalf,
        proc_send: ProcSender,
        early_send: EarlyReplySender,
    ) -> JoinHandle<()> {
        tokio::spawn(Self { readhalf, proc_send, early_send }.run())
    }

    async fn run(self) {
        todo!("Implement ReadTask")
    }
}
