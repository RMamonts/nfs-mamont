use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;

/// Writes [`crate::vfs_task::VfsTask`] responses to a network connection.
pub struct WriteTask {
    _writehalf: OwnedWriteHalf,
    _result_receiver: UnboundedReceiver<()>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn new(writehalf: OwnedWriteHalf, result_receiver: UnboundedReceiver<()>) -> Self {
        Self { _writehalf: writehalf, _result_receiver: result_receiver }
    }

    /// Spawns a [`WriteTask`]  that writes command results to a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        todo!("Implement WriteTask")
    }
}
