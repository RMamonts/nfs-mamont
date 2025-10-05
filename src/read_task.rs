use std::io;

use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;

/// Reads RPC commands from a network connection, parses it,
/// and forwards them to a [`crate::vfs_task::VfsTask`].
pub struct ReadTask {
    _readhalf: OwnedReadHalf,
    _command_sender: UnboundedSender<()>,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn new(readhalf: OwnedReadHalf, command_sender: UnboundedSender<()>) -> Self {
        Self { _readhalf: readhalf, _command_sender: command_sender }
    }

    /// Spawns a [`ReadTask`]  that reads commands from a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) -> io::Result<()> {
        todo!("Implement ReadTask")
    }
}
