pub mod compound_processor;

use drr_channel::{self, Cost, Slot};
use std::future::Future;
use std::pin::Pin;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

use crate::allocator::Buffer;
use crate::limits::budget::ReplySender;
use crate::op_context::nfsv4::{ClientContext, CompoundContext};
use crate::task::global::mount::{MountCommand, MountTaskSender};
use crate::task::global::nfsv4::{Nfs4Arguments, Nfsv4Command, Nfsv4Op};
use crate::task::global::nlm::nlm_task::{NlmCommand, NlmTaskSender};
use crate::task::global::vfs::Nfs3Command;
use crate::task::{ProcReply, ProcResult};
use crate::vfs;
use crate::vfs::nfsv4::compound::OperationRes;
use crate::vfs::nfsv4::Error;

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

type CompoundFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

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
    compound_sender: UnboundedSender<CompoundFuture>,
}

impl<B> ManagerTask<B>
where
    B: Buffer + 'static,
{
    pub fn new(
        receiver: drr_channel::Receiver<ManagerCommand<B>>,
        mount_sender: MountTaskSender<B>,
        nlm_sender: NlmTaskSender<B>,
    ) -> (Self, ManagerTaskReceiver<B>, tokio::sync::mpsc::UnboundedReceiver<CompoundFuture>) {
        // TODO: replace with crossbeam_queue or other spmc?
        let (sender, worker_recv) = async_channel::unbounded::<ManagerMessage<B>>();
        let (compound_sender, recv_compound) =
            tokio::sync::mpsc::unbounded_channel::<CompoundFuture>();
        let task = Self {
            receiver,
            mount_sender,
            nlm_sender,
            sender,
            context: ClientContext::new(),
            compound_sender,
        };
        (task, worker_recv, recv_compound)
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
                    match command.args.op {
                        Nfs4Arguments::Null => {
                            // TODO: handle normally
                            let _ = command
                                .result_tx
                                .send(ProcReply {
                                    xid: command.args.header.xid,
                                    proc_result: Ok(ProcResult::Nfs4(Box::new(
                                        vfs::nfsv4::NfsRes::Null,
                                    ))),
                                })
                                .await;
                        }
                        Nfs4Arguments::Compound(args) => {
                            let context = CompoundContext::new(
                                command.args.header.xid,
                                args.tag,
                                command.args.header.cred,
                                args.argarray.into_iter().collect(),
                            );
                            let sender = self.sender.clone();

                            let fut = new_future(context, command.result_tx, sender);

                            let _ = self.compound_sender.send(Box::pin(fut));
                        }
                    }
                    drop(slot);
                }
            }
        }
    }
}

async fn new_future<B: Buffer + 'static>(
    mut context: CompoundContext<B>,
    result_tx: ReplySender<B>,
    sender: ManagerTaskSender<B>,
) {
    let mut results = Vec::with_capacity(context.ops.len());
    loop {
        let Some(next) = context.ops.pop_front() else {
            break;
        };
        let (callback_sender, callback_recv) =
            oneshot::channel::<vfs::nfsv4::compound::OperationRes<B>>();
        let operation = Nfsv4Op { operation: next, callback: callback_sender };
        // TODO: handle properly
        let _ = sender.send(ManagerMessage::NFSv4(operation)).await;
        let Ok(res) = callback_recv.await else {
            // handle properly
            break;
        };

        let result = convert_to_result(&res);
        results.push(res);

        if result.is_err() {
            let _ = result_tx
                .send(ProcReply {
                    xid: context.xid,
                    proc_result: Ok(ProcResult::Nfs4(Box::new(vfs::nfsv4::NfsRes::Compound {
                        tag: context.tag,
                        status: result,
                        result_array: results,
                    }))),
                })
                .await;
            return;
        }
    }
    let _ = result_tx
        .send(ProcReply {
            xid: context.xid,
            proc_result: Ok(ProcResult::Nfs4(Box::new(vfs::nfsv4::NfsRes::Compound {
                tag: context.tag,
                status: Result::Ok(()),
                result_array: results,
            }))),
        })
        .await;
}

pub fn convert_to_result<B: Buffer>(res: &OperationRes<B>) -> Result<(), Error> {
    match res {
        OperationRes::Access(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Close(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Commit(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Create(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::DelegPurge(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::DelegReturn(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::GetAttr(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::GetFh(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Link(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Lock(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::LockT(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::LockU(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Lookup(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::LookupP(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::NVerify(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Open(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::OpenAttr(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::OpenConfirm(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::OpenDowngrade(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::PutFh(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::PutPubFh(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::PutRootFh(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Read(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::ReadDir(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::ReadLink(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Remove(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Rename(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Renew(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::RestoreFh(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::SaveFh(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Secinfo(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::SetAttr(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::SetClientId(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::SetClientIdConfirm(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Verify(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::Write(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        OperationRes::ReleaseLockOwner(res) => match res {
            Ok(_) => Ok(()),
            Err(err) => Err(err.error),
        },
        // TODO: check, what should be here
        OperationRes::Illegal => Ok(()),
    }
}
