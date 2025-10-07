use crate::message_types::{Procedure, Reply};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

/// Process RPC commands,sends operation results to [`crate::stream_writer::StreamWriter`].
pub struct VfsTask {
    receiver: UnboundedReceiver<Procedure>,
    sender: UnboundedSender<Reply>,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn spawn(
        receiver: UnboundedReceiver<Procedure>,
        sender: UnboundedSender<Reply>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move { Self { receiver, sender }.run().await })
    }

    async fn run(mut self) {
        while let Some(_) = self.receiver.recv().await {
            todo!("Do something with request")
        }
    }
}
