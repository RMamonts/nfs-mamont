use drr_channel::{Receiver, Slot};

use crate::task::global::vfs::VfsCommand;
use crate::Buffer;

pub type ManagerTaskSender<B> = async_channel::Sender<ManagerMessage<B>>;
pub type ManagerTaskReceiver<B> = async_channel::Receiver<ManagerMessage<B>>;

pub struct ManagerMessage<B: Buffer> {
    pub command: VfsCommand<B>,
    //TODO: change to actual message
    pub slot: Slot,
}
pub struct ManagerTask<B>
where
    B: Buffer,
{
    vfs_recv: Receiver<VfsCommand<B>>,
    sender: ManagerTaskSender<B>,
}

impl<B> ManagerTask<B>
where
    B: Buffer + 'static,
{
    pub fn new(vfs_recv: Receiver<VfsCommand<B>>) -> (Self, ManagerTaskReceiver<B>) {
        /// TODO: replace with crossbeam_queue or other spmc?
        let (sender, receiver) = async_channel::unbounded::<ManagerMessage<B>>();
        let task = Self { vfs_recv, sender };
        (task, receiver)
    }

    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }
    async fn run(self) {
        while let Some((command, slot)) = self.vfs_recv.recv().await {
            // TODO: for now we just retransmit request to VFS pool and drop slo
            let cmd = ManagerMessage { command, slot };
            // TODO: need to handle properly
            let _ = self.sender.send(cmd).await;
            // callback actually required to handle nfsv4 compound processing order
            // TODO: after VfsCommand extended with nfsv4, there should be callback to send next op
        }
    }
}
