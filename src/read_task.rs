use tokio::net::tcp::OwnedReadHalf;
use tokio::task::JoinHandle;

use crate::vfs_task;

/// Reads RPC commands from a network connection, parses it,
/// and forwards them to a [`crate::vfs_task::VfsTask`].
pub struct ReadTask {
    readhalf: OwnedReadHalf,
    vfs_task: vfs_task::WriteHalf,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn spawn(readhalf: OwnedReadHalf, vfs_task: vfs_task::WriteHalf) -> JoinHandle<()> {
        let read_task = Self { readhalf, vfs_task };

        tokio::spawn(read_task.run())
    }

    async fn run(self) {
        todo!("Implement ReadTask")
    }
}
