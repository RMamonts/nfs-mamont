use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use std::sync::Arc;

use crate::vfs::Vfs;

/// Process RPC commands,sends operation results to [`crate::write_task::WriteTask`].
pub struct VfsTask<V: Vfs + 'static> {
    _command_receiver: UnboundedReceiver<()>,
    _result_sender: UnboundedSender<()>,
    _vfs: Arc<V>,
}

impl<V: Vfs + 'static> VfsTask<V> {
    /// Creates new instance of [`VfsTask`].
    pub fn new(
        command_receiver: UnboundedReceiver<()>,
        result_sender: UnboundedSender<()>,
        vfs: Arc<V>,
    ) -> Self {
        Self {
            _command_receiver: command_receiver,
            _result_sender: result_sender,
            _vfs: vfs,
        }
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
