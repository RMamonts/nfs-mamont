use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// Process RPC commands,sends operation results to [`crate::write_task::WriteTask`].
pub struct VfsTask {
    _command_receiver: UnboundedReceiver<()>,
    _result_sender: UnboundedSender<()>,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn new(
        command_receiver: UnboundedReceiver<()>,
        result_sender: UnboundedSender<()>,
    ) -> Self {
        Self { _command_receiver: command_receiver, _result_sender: result_sender }
    }

    /// Spawns a [`VfsTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        todo!("Implement VfsTask")
    }
}
