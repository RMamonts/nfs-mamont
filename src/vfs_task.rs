use crate::message_types::{Request, Response};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

/// Process RPC commands,sends operation results to [`crate::write_task::WriteTask`].
pub struct VfsTask {
    receiver: UnboundedReceiver<Request>,
    sender: UnboundedSender<Response>,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn spawn(
        receiver: UnboundedReceiver<Request>,
        sender: UnboundedSender<Response>,
    ) -> JoinHandle<()> {
        let vfs_task = Self { receiver, sender };
        tokio::spawn(async move { vfs_task.run().await })
    }

    async fn run(mut self) {
        loop {
            match self.receiver.recv().await {
                None => {}
                Some(_) => {}
            }
        }
    }
}
