pub mod compound_processor;

use tokio::sync::mpsc::UnboundedSender;

use drr_channel::{self, Cost, Slot};

use crate::allocator::Buffer;
use crate::op_context::nfsv4::ClientContext;
use crate::task::global::mount::{MountCommand, MountTaskSender};
use crate::task::global::nfsv4::{Nfsv4Command, Nfsv4Op};
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
    Nfsv3(Nfs3Command<B>),
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
            Self::Nfsv3(command) => command.cost(),
            Self::Mount(_) | Self::Nlm(_) | Self::Nfsv4(_) => 1,
        }
    }
}

/// NFS command handed to the VFS pool workers together with the slot of its queue spot.
pub enum ManagerMessage<B: Buffer> {
    NFSv3 {
        cmd: Nfs3Command<B>,
        slot: Slot,
    },
    #[allow(unused)]
    NFSv4(Nfsv4Op<B>),
}

pub struct ManagerTask<B>
where
    B: Buffer,
{
    receiver: drr_channel::Receiver<ManagerCommand<B>>,
    mount_sender: MountTaskSender<B>,
    nlm_sender: NlmTaskSender<B>,
    sender: ManagerTaskSender<B>,
    #[allow(unused)]
    context: ClientContext,
    compound_sender: UnboundedSender<(Nfsv4Command<B>, Slot)>,
}

impl<B> ManagerTask<B>
where
    B: Buffer + 'static,
{
    pub fn new(
        receiver: drr_channel::Receiver<ManagerCommand<B>>,
        mount_sender: MountTaskSender<B>,
        nlm_sender: NlmTaskSender<B>,
        sender: ManagerTaskSender<B>,
    ) -> (Self, tokio::sync::mpsc::UnboundedReceiver<(Nfsv4Command<B>, Slot)>) {
        // TODO: replace with crossbeam_queue or other spmc?
        let (compound_sender, recv_compound) =
            tokio::sync::mpsc::unbounded_channel::<(Nfsv4Command<B>, Slot)>();
        let task = Self {
            receiver,
            mount_sender,
            nlm_sender,
            sender,
            context: ClientContext::new(),
            compound_sender,
        };
        (task, recv_compound)
    }

    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }
    async fn run(self) {
        while let Some((command, slot)) = self.receiver.recv().await {
            match command {
                ManagerCommand::Nfsv3(cmd) => {
                    let msg = ManagerMessage::NFSv3 { cmd, slot };
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
                    let _ = self.compound_sender.send((command, slot));
                }
            }
        }
    }
}
