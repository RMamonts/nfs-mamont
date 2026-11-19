use drr_channel::{Cost, Receiver, Slot};

use crate::allocator::Buffer;
use crate::op_context::nfsv4::ClientContext;
use crate::task::global::mount::{MountCommand, MountTaskSender};
use crate::task::global::nfsv4::Nfsv4Command;
use crate::task::global::nlm::nlm_task::{NlmCommand, NlmTaskSender};
use crate::task::global::vfs::Nfs3Command;

pub type ManagerTaskSender<B> = async_channel::Sender<ManagerMessage<B>>;
pub type ManagerTaskReceiver<B> = async_channel::Receiver<ManagerMessage<B>>;

/// A parsed RPC command of any protocol the task manager serves.
///
/// One channel between connection read tasks and the [`ManagerTask`] carries all of
/// them, so that the manager owns the senders to every protocol handler.
pub enum ManagerCommand<B: Buffer> {
    /// An NFSv3 procedure forwarded to the VFS pool.
    Nfs(Nfs3Command<B>),
    /// A MOUNT procedure forwarded to the mount task.
    Mount(MountCommand<B>),
    /// An NLMv4 procedure forwarded to the NLM task.
    Nlm(NlmCommand<B>),
    /// An NFSv4 procedure forwarded to the NFSv4 task (not yet handled).
    #[allow(unused)]
    Nfsv4(Nfsv4Command<B>),
}

/// Flow sender of [`ManagerCommand`]: one flow per connection.
pub type ManagerCommandSender<B> = drr_channel::FlowSender<ManagerCommand<B>>;

impl<B: Buffer> Cost for ManagerCommand<B> {
    fn cost(&self) -> u32 {
        match self {
            Self::Nfs(command) => command.cost(),
            Self::Mount(_) | Self::Nlm(_) | Self::Nfsv4(_) => 1,
        }
    }
}

/// NFS command handed to the VFS pool workers together with the slot of its queue spot.
pub struct ManagerMessage<B: Buffer> {
    pub command: Nfs3Command<B>,
    pub slot: Slot,
}

pub struct ManagerTask<B>
where
    B: Buffer,
{
    receiver: Receiver<ManagerCommand<B>>,
    mount_sender: MountTaskSender<B>,
    nlm_sender: NlmTaskSender<B>,
    sender: ManagerTaskSender<B>,
    #[allow(unused)]
    context: ClientContext,
}

impl<B> ManagerTask<B>
where
    B: Buffer + 'static,
{
    pub fn new(
        receiver: Receiver<ManagerCommand<B>>,
        mount_sender: MountTaskSender<B>,
        nlm_sender: NlmTaskSender<B>,
    ) -> (Self, ManagerTaskReceiver<B>) {
        // TODO: replace with crossbeam_queue or other spmc?
        let (sender, worker_recv) = async_channel::unbounded::<ManagerMessage<B>>();
        let task =
            Self { receiver, mount_sender, nlm_sender, sender, context: ClientContext::new() };
        (task, worker_recv)
    }

    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }
    async fn run(self) {
        while let Some((command, slot)) = self.receiver.recv().await {
            match command {
                ManagerCommand::Nfs(command) => {
                    let msg = ManagerMessage { command, slot };
                    // TODO: need to handle properly
                    let _ = self.sender.send(msg).await;
                }
                ManagerCommand::Mount(command) => {
                    let xid = command.args.header.xid;
                    if let Err(command) = self.mount_sender.send(command).await {
                        let _ = command.0.result_tx.broken_pipe(xid).await;
                    }
                    // TODO: maybe find a way to handle it without explicit slot?
                    drop(slot);
                }
                ManagerCommand::Nlm(command) => {
                    let xid = command.args.header.xid;
                    if let Err(command) = self.nlm_sender.send(command).await {
                        let _ = command.0.result_sender.broken_pipe(xid).await;
                    }
                    drop(slot);
                }
                ManagerCommand::Nfsv4(command) => {
                    // TODO: handle nfsv4
                    let _ = command;
                    drop(slot);
                }
            }
        }
    }
}
