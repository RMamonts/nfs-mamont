use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::mount::{ExportEntry, MountEntry};
use crate::parser::{MountArgWrapper};
use crate::task::ProcReply;
use crate::vfs::file;

/// Command sent to [`MountTask`] from connection read tasks.
pub struct MountCommand {
    /// Channel used to pass the result to write task.
    pub result_tx: UnboundedSender<ProcReply>,
    /// Placeholder for mount procedure args.
    pub args: MountArgWrapper,
}

#[derive(Default)]
struct ExportRegistry {
    // one dir can have only one export
    #[allow(dead_code)]
    by_directory: HashMap<file::Path, ExportEntry>,
}

#[derive(Default)]
struct MountRegistry {
    // one client can mount multiple dirs
    #[allow(dead_code)]
    by_client: HashMap<SocketAddr, HashSet<MountEntry>>,
}

struct MountContext {
    // what's available to mount
    #[allow(dead_code)]
    exports: ExportRegistry,
    // who has mounted what
    #[allow(dead_code)]
    mounts: MountRegistry,
    // channel for commands from client connection tasks
    receiver: UnboundedReceiver<MountCommand>,
}

pub struct MountTask {
    #[allow(dead_code)]
    context: MountContext,
}

impl MountTask {
    /// Creates new instance of [`MountTask`]
    pub fn new() -> (Self, UnboundedSender<MountCommand>) {
        let (sender, receiver) = mpsc::unbounded_channel::<MountCommand>();

        let task = Self {
            context: MountContext {
                exports: ExportRegistry::default(),
                mounts: MountRegistry::default(),
                receiver,
            },
        };

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
        let mut receiver = self.context.receiver;

        while let Some(_command) = receiver.recv().await {
            // Send result back. It's fine if write task is already dropped.
            // TODO("https://github.com/RMamonts/nfs-mamont/issues/123"
            //let _ = command.result_tx.send();
        }
    }
}
