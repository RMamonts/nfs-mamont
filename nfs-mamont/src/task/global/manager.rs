use std::future::Future;
use std::pin::Pin;

use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::sync::oneshot;
use tracing::warn;

use drr_channel::{Cost, Receiver, Slot};

use crate::allocator::Buffer;
use crate::limits::budget::ReplySender;
use crate::op_context::nfsv4::{ClientContext, CompoundContext};
use crate::parser::{Nfs3ArgWrapper, RpcHeader};
use crate::task::global::mount::{MountCommand, MountTaskSender};
use crate::task::global::nfsv4::{Nfs4Arguments, Nfsv4Command};
use crate::task::global::nlm::nlm_task::{NlmCommand, NlmTaskSender};
use crate::task::global::vfs::Nfs3Command;
use crate::task::{ProcReply, ProcResult};
use crate::vfs;

pub type ManagerTaskSender<B> = async_channel::Sender<ManagerArgs<B>>;
pub type ManagerTaskReceiver<B> = async_channel::Receiver<ManagerArgs<B>>;

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

pub enum ManagerArgs<B: Buffer> {
    NFSv3(ManagerNfsv3Command<B>),
    #[allow(unused)]
    NFSv4(ManagerNfsv4Command<B>),
}

pub struct ManagerNfsv3Command<B: Buffer> {
    pub reply_sender: ReplySender<B>,
    pub args: Nfs3ArgWrapper<B>,
    pub slot: Slot,
}

#[allow(unused)]
pub struct ManagerNfsv4Command<B: Buffer> {
    reply_sender: oneshot::Sender<vfs::nfsv4::compound::OperationRes<B>>,
    args: vfs::nfsv4::compound::OperationArgs<B>,
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

type CompoundFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

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
        let (sender, worker_recv) = async_channel::unbounded::<ManagerArgs<B>>();
        let task =
            Self { receiver, mount_sender, nlm_sender, sender, context: ClientContext::new() };
        (task, worker_recv)
    }

    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }
    async fn run(self) {
        let mut futures = futures::stream::FuturesUnordered::new();
        let mut is_closed = false;

        loop {
            tokio::select! {
                new_cmd = self.receiver.recv(), if !is_closed => {
                    match new_cmd {
                        // all readers are closed and all msgs are processed
                        None => {
                            is_closed = true;
                        },
                        Some((command, slot)) => {
                            self.handle_proc(command, slot, &mut futures).await;
                        }
                    }
                }
                _ = futures.next(), if !futures.is_empty() => {}
                else => break,
            }
        }
    }

    async fn handle_proc(
        &self,
        command: ManagerCommand<B>,
        slot: Slot,
        futures: &mut FuturesUnordered<CompoundFuture>,
    ) {
        match command {
            ManagerCommand::Nfsv3(cmd) => {
                let msg = ManagerArgs::NFSv3(ManagerNfsv3Command {
                    reply_sender: cmd.result_tx,
                    args: cmd.args,
                    slot,
                });
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
                match command.args.op {
                    Nfs4Arguments::Null => {
                        unimplemented!()
                    }
                    Nfs4Arguments::Compound(args) => {
                        let fut = new_future(
                            args,
                            command.args.header,
                            command.result_tx,
                            self.sender.clone(),
                        );
                        futures.push(Box::pin(fut));
                    }
                }
                drop(slot);
            }
        }
    }
}

async fn new_future<B: Buffer>(
    args: vfs::nfsv4::compound::Args<B>,
    header: RpcHeader,
    result_tx: ReplySender<B>,
    sender: ManagerTaskSender<B>,
) {
    let mut context = CompoundContext::new(header.xid, args.tag, header.cred, args.argarray.into());
    let mut results = Vec::with_capacity(context.ops.len());
    let result = loop {
        let Some(next) = context.ops.pop_front() else { break Ok(()) };
        let (callback_sender, callback_recv) =
            oneshot::channel::<vfs::nfsv4::compound::OperationRes<B>>();
        // TODO: handle properly
        let _ = sender
            .send(ManagerArgs::NFSv4(ManagerNfsv4Command {
                reply_sender: callback_sender,
                args: next,
            }))
            .await;

        let Ok(res) = callback_recv.await else {
            // callback was not send, which could happen on unexpected worker task drop
            break Err(vfs::nfsv4::Error::ServerFault);
        };

        let result = res.extract_result();
        results.push(res);

        if result.is_err() {
            break result;
        }
    };
    let reply = ProcReply {
        xid: context.xid,
        proc_result: Ok(ProcResult::Nfs4(Box::new(vfs::nfsv4::NfsRes::Compound {
            tag: context.tag,
            status: result,
            result_array: results,
        }))),
    };
    match result_tx.send(reply).await {
        Ok(_) => {}
        Err(e) => warn!("Failed to send reply: {:?}", e),
    }
}
