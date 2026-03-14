use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::task::ProcReply;

/// Process RPC commands, sends operation results to [`crate::task::connection::write::WriteTask`].
pub struct VfsTask {
    _command_receiver: UnboundedReceiver<()>,
    _result_sender: UnboundedSender<ProcReply>,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn new(
        command_receiver: UnboundedReceiver<()>,
        result_sender: UnboundedSender<ProcReply>,
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
        todo!("https://github.com/RMamonts/nfs-mamont/issues/122")
    }
}
