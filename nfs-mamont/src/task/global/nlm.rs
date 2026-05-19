use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::debug;

use crate::nlm::Nlm;
use crate::task::{ProcReply, ProcResult};
use crate::{
    nlm::NlmRes,
    parser::{NlmArgWrapper, NlmArguments},
};

pub struct NlmCommand {
    /// Channel used to pass the result to write task.
    pub result_tx: UnboundedSender<ProcReply>,
    /// Client socket address from connection task.
    pub client_addr: SocketAddr,
    /// Placeholder for NLM procedure args.
    pub args: NlmArgWrapper,
}

pub struct NlmTask<N>
where
    N: Nlm + Send + Sync + 'static,
{
    nlm_service: Arc<N>,

    // Channel for commands from client connection tasks
    receiver: UnboundedReceiver<NlmCommand>,
}

impl<N> NlmTask<N>
where
    N: Nlm + Send + Sync + 'static,
{
    /// Creates new instance of [`NlmTask`]
    pub fn new(nlm_service: Arc<N>) -> (Self, UnboundedSender<NlmCommand>) {
        let (sender, receiver) = mpsc::unbounded_channel::<NlmCommand>();

        let task = Self { nlm_service, receiver };

        (task, sender)
    }

    /// Spawns a [`NlmTask`]  that processes nlm commands received from
    /// `ReadTask` and returns results to `WriteTask`.
    ///
    /// # Panics
    ///
    /// If called outside tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let nlm_service = self.nlm_service;
        let mut receiver = self.receiver;

        while let Some(command) = receiver.recv().await {
            let NlmCommand { result_tx, client_addr, args } = command;
            let NlmArgWrapper { header, proc } = args;
            debug!(xid = header.xid, "nlm task: command received");

            let nlm_result = match *proc {
                NlmArguments::Null => NlmRes::Null,
                NlmArguments::Lock(Nlm4LockArgs) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM");
                    let res = nlm_service.lock(Nlm4LockArgs, client_addr, header.cred).await;
                    NlmRes::Lock(Box::new(res))
                }
                NlmArguments::Unlock(Nlm4UnlockArgs) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM");
                    let res = nlm_service.unlock(Nlm4UnlockArgs, client_addr, header.cred).await;
                    NlmRes::Unlock(res)
                }
                NlmArguments::Test(Nlm4TestArgs) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM");
                    let res = nlm_service.test(Nlm4TestArgs, client_addr, header.cred).await;
                    NlmRes::Test(Box::new(res))
                }
                NlmArguments::Cancel(Nlm4CancelArgs) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM");
                    let res = nlm_service.cancel(Nlm4CancelArgs, client_addr, header.cred).await;
                    NlmRes::Cancel(res)
                }
            };

            // TODO:
            // - some logs when occurred error
            // - or retry with fail
            // * but don't stop task
            let _ = result_tx.send(ProcReply {
                xid: header.xid,
                proc_result: Ok(ProcResult::Nlm4(Box::new(nlm_result))),
            });
            debug!(xid = header.xid, "nlm task: reply queued");
        }
    }
}
