use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

/// Process RPC commands,sends operation results to [`crate::write_task::WriteTask`].
pub struct VfsTask {
    receiver: UnboundedReceiver<Request>,
    sender: UnboundedSender<Response>,
}

pub struct WriteHalf {
    sender: UnboundedSender<Request>,
}

pub enum Request {
    NFSCommand,
    Cancel,
}

impl WriteHalf {
    async fn cancel(&mut self) {
        let _ = self.sender.send(Request::Cancel);
    }

    async fn nfs(&mut self) {
        let _ = self.sender.send(Request::NFSCommand);
    }
}

pub enum Response {}

pub struct ReadHalf {
    receiver: UnboundedReceiver<Response>,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn spawn() -> (WriteHalf, JoinHandle<()>, ReadHalf) {
        let (write_sender, write_receiver) = mpsc::unbounded_channel::<Request>();
        let (read_sender, read_receiver) = mpsc::unbounded_channel::<Response>();

        let vfs_task = Self { receiver: write_receiver, sender: read_sender };
        let join_handle = tokio::spawn(async move { vfs_task.run().await });

        let write_half = WriteHalf { sender: write_sender };
        let read_half = ReadHalf { receiver: read_receiver };

        (write_half, join_handle, read_half)
    }

    async fn run(mut self) {
        loop {
            match self.receiver.recv().await.unwrap() {
                Request::Cancel => return,
                Request::NFSCommand => todo!("Implement NFS"),
            }
        }
    }
}
