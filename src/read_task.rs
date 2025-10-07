use crate::message_types::{EarlyReply, Request};
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

/// Reads RPC commands from a network connection, parses it,
/// and forwards them to a [`crate::vfs_task::VfsTask`].
pub struct ReadTask {
    readhalf: OwnedReadHalf,
    request_send: UnboundedSender<Request>,
    early_send: UnboundedSender<EarlyReply>,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn spawn(
        readhalf: OwnedReadHalf,
        request_send: UnboundedSender<Request>,
        early_send: UnboundedSender<EarlyReply>,
    ) -> JoinHandle<()> {
        tokio::spawn(Self { readhalf, request_send, early_send }.run())
    }

    async fn run(self) {
        todo!("Implement ReadTask")
    }
}
