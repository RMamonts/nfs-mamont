use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;

use crate::mount::{ExportEntry, MountEntry};
use crate::vfs::file;

// sender - to send to the WriteTask
// reciever to recieve from ReadTask
type Link = (UnboundedSender<()>, UnboundedReceiver<()>);

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
    // to send chenels for client connections
    reciever: UnboundedReceiver<Link>,
}

pub struct MountTask {
    #[allow(dead_code)]
    context: MountContext,
}

impl MountTask {
    /// Creates new instance of [`MountTask`]
    pub fn new() -> (Self, UnboundedSender<Link>) {
        let (sender, reciever) = mpsc::unbounded_channel::<Link>();

        let task = Self {
            context: MountContext {
                exports: Arc::new(RwLock::new(ExportRegistry::default())),
                mounts: Arc::new(RwLock::new(MountRegistry::default())),
                reciever,
            },
        };

        (task, sender)
    }

    /// Spawns a [`MountTask`]  that processes mount commands recieved from
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
        let mut reciever = self.context.reciever;

        loop {
            // get onehot tx
            let (tx, mut rx) = reciever.recv().await.unwrap();

            // TODO: cancelation token or notify if client session ends
            tokio::spawn(async move {
                loop {
                    // TODO:
                    // - recieve command
                    rx.recv().await.unwrap();
                    // - process mount request
                    // ...
                    // - send result back
                    tx.send(()).unwrap();
                }
            });
        }
    }
}
