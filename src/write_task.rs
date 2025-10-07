use crate::message_types::{EarlyReply, Response};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;

/// Writes [`vfs_task::VfsTask`] responses to a network connection.
pub struct WriteTask {
    writehalf: OwnedWriteHalf,
    request_recv: UnboundedReceiver<Response>,
    early_recv: UnboundedReceiver<EarlyReply>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn spawn(
        writehalf: OwnedWriteHalf,
        request_recv: UnboundedReceiver<Response>,
        early_recv: UnboundedReceiver<EarlyReply>,
    ) -> JoinHandle<()> {
        tokio::spawn(Self { writehalf, request_recv, early_recv }.run())
    }

    async fn run(self) {
        todo!("Implement WriteTask")
    }
}
