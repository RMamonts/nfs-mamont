use crate::task::ProcReply;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;

/// Writes [`crate::task::connection::vfs::VfsTask`] responses to a network connection.
pub struct WriteTask {
    _writehalf: OwnedWriteHalf,
    _result_receiver: UnboundedReceiver<ProcReply>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn new(writehalf: OwnedWriteHalf, result_receiver: UnboundedReceiver<ProcReply>) -> Self {
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
        todo!("https://github.com/RMamonts/nfs-mamont/issues/121")
    }
}
