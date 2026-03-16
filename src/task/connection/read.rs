use std::io;

use crate::parser::NfsArgWrapper;
use crate::task::global::mount::MountCommand;
use crate::task::ProcReply;
use std::net::SocketAddr;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;

/// Reads RPC commands from a network connection, parses them,
/// and forwards to [`crate::task::connection::vfs::VfsTask`] or global tasks.
#[allow(dead_code)]
pub struct ReadTask {
    readhalf: OwnedReadHalf,
    client_addr: SocketAddr,
    command_sender: UnboundedSender<NfsArgWrapper>,
    // to send messages into mount task
    mount_sender: UnboundedSender<MountCommand>,
    // to pass into mount task as part of message,
    // so mount task can send result back to write task
    // and
    // to bypass vfs with null procedure
    result_sender: UnboundedSender<ProcReply>,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn new(
        readhalf: OwnedReadHalf,
        client_addr: SocketAddr,
        command_sender: UnboundedSender<NfsArgWrapper>,
        mount_sender: UnboundedSender<MountCommand>,
        result_sender: UnboundedSender<ProcReply>,
    ) -> Self {
        Self { readhalf, client_addr, command_sender, mount_sender, result_sender }
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
        todo!("https://github.com/RMamonts/nfs-mamont/issues/120")
    }
}
