use std::future::Future;
use std::pin::Pin;

use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::oneshot;

use drr_channel::Slot;

use crate::limits::budget::ReplySender;
use crate::op_context::nfsv4::CompoundContext;
use crate::task::global::manager::{ManagerMessage, ManagerTaskSender};
use crate::task::global::nfsv4::{Nfs4Arguments, Nfsv4Command, Nfsv4Op};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::nfsv4::compound::OperationRes;
use crate::vfs::nfsv4::Error;
use crate::{vfs, Buffer};

pub type CompoundFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

pub struct Nfsv4Processor<B: Buffer> {
    max_recv: usize,
    receiver: UnboundedReceiver<(Nfsv4Command<B>, Slot)>,
    futures: FuturesUnordered<CompoundFuture>,
    sender: ManagerTaskSender<B>,
    _marker: std::marker::PhantomData<B>,
}

impl<B: Buffer + 'static> Nfsv4Processor<B> {
    pub fn new(
        max_recv: usize,
        receiver: UnboundedReceiver<(Nfsv4Command<B>, Slot)>,
        sender: ManagerTaskSender<B>,
    ) -> Self {
        Self {
            receiver,
            futures: FuturesUnordered::new(),
            max_recv,
            _marker: std::marker::PhantomData,
            sender,
        }
    }

    pub fn spawn(self) {
        tokio::task::spawn(async move { self.run().await });
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                Some((cmd, slot)) = self.receiver.recv(), if self.futures.len() < self.max_recv  => {
                    self.dispatch(cmd).await;
                    drop(slot);
                },
                Some(_) = self.futures.next() => {}
            }
        }
    }

    async fn dispatch(&mut self, command: Nfsv4Command<B>) {
        match command.args.op {
            Nfs4Arguments::Null => {
                // TODO: handle normally
                let _ = command
                    .result_tx
                    .send(ProcReply {
                        xid: command.args.header.xid,
                        proc_result: Ok(ProcResult::Nfs4(Box::new(vfs::nfsv4::NfsRes::Null))),
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

                self.futures.push(Box::pin(fut));
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
