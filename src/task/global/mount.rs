use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;

use crate::mount::{ExportEntry, MountEntry};
use crate::vfs::file;

/// Command sent to [`MountTask`] from connection read tasks.
pub struct MountCommand {
    /// Channel used to pass the result to write task.
    pub result_tx: UnboundedSender<()>,
    /// Placeholder for mount procedure args.
    pub args: (),
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
    exports: Arc<RwLock<ExportRegistry>>,
    // who has mounted what
    #[allow(dead_code)]
    mounts: Arc<RwLock<MountRegistry>>,
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
                exports: Arc::new(RwLock::new(ExportRegistry::default())),
                mounts: Arc::new(RwLock::new(MountRegistry::default())),
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

        while let Some(command) = receiver.recv().await {
            // Send result back. It's fine if write task is already dropped.
            let _ = command.result_tx.send(());
        }
    }
}
