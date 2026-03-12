use std::io;

use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;

use crate::task::global::mount::MountCommand;

/// Reads RPC commands from a network connection, parses them,
/// and forwards to [`crate::task::connection::vfs::VfsTask`] or global tasks.
pub struct ReadTask {
    _readhalf: OwnedReadHalf,
    _command_sender: UnboundedSender<()>,
    // to pass into mount task
    _mount_sender: UnboundedSender<MountCommand>,
    // to pass into mount task
    _result_sender: UnboundedSender<()>,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn new(
        readhalf: OwnedReadHalf,
        command_sender: UnboundedSender<()>,
        mount_sender: UnboundedSender<MountCommand>,
        result_sender: UnboundedSender<()>,
    ) -> Self {
        Self {
            _readhalf: readhalf,
            _command_sender: command_sender,
            _mount_sender: mount_sender,
            _result_sender: result_sender,
        }
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
