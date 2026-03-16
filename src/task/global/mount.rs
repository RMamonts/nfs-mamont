use std::net::SocketAddr;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::context::SharedVfs;
use crate::mount::ExportEntry;
use crate::parser::MountArgWrapper;
use crate::service::mount::MountService;
use crate::task::ProcReply;

/// Command sent to [`MountTask`] from connection read tasks.
pub struct MountCommand {
    /// Channel used to pass the result to write task.
    pub result_tx: UnboundedSender<ProcReply>,
    /// Client socket address from connection task.
    pub client_addr: SocketAddr,
    /// Placeholder for mount procedure args.
    pub args: MountArgWrapper,
}

pub struct MountTask {
    #[allow(dead_code)]
    mount_service: MountService,
    // channel for commands from client connection tasks
    receiver: UnboundedReceiver<MountCommand>,
}

impl MountTask {
    /// Creates new instance of [`MountTask`]
    pub fn new(exports: Vec<ExportEntry>, vfs: SharedVfs) -> (Self, UnboundedSender<MountCommand>) {
        let (sender, receiver) = mpsc::unbounded_channel::<MountCommand>();

        let task = Self { mount_service: MountService::with_exports(exports, vfs), receiver };

        (task, sender)
    }

    /// Spawns a [`MountTask`]  that processes mount commands received from
    /// [`crate::task::connection::read::ReadTask`] and returns results to
    /// [`crate::task::connection::write::WriteTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let mut receiver = self.receiver;

        while let Some(_command) = receiver.recv().await {
            // Send result back. It's fine if write task is already dropped.
            // TODO("https://github.com/RMamonts/nfs-mamont/issues/123"
            //let _ = command.result_tx.send();
        }
    }
}
