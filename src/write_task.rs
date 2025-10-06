use tokio::net::tcp::OwnedWriteHalf;
use tokio::task::JoinHandle;

use crate::vfs_task;

/// Writes [`crate::vfs_task::VfsTask`] responses to a network connection.
pub struct WriteTask {
    _writehalf: OwnedWriteHalf,
    vfs_task: vfs_task::ReadHalf,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn spawn(writehalf: OwnedWriteHalf, vfs_task: vfs_task::ReadHalf) -> JoinHandle<()> {
        tokio::spawn(Self { _writehalf: writehalf, vfs_task }.run())
    }

    async fn run(self) {
        todo!("Implement WriteTask")
    }
}
