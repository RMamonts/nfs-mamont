#[allow(dead_code)]
use tokio::task::JoinHandle;

use crate::message_types::{ProcRecv, ReplySender};

/// Process RPC commands,sends operation results to [`crate::stream_writer::StreamWriter`].
pub struct VfsTask {
    proc_recv: ProcRecv,
    reply_sender: ReplySender,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn spawn(proc_recv: ProcRecv, reply_sender: ReplySender) -> JoinHandle<()> {
        tokio::spawn(async move { Self { proc_recv, reply_sender }.run().await })
    }

    #[allow(clippy::redundant_pattern_matching)]
    async fn run(mut self) {
        while let Some(_) = self.proc_recv.recv().await {
            todo!("Do something with request")
        }
    }
}
