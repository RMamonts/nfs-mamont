//! NLMv4 task dispatcher.
//!
//! Runs a background task that receives parsed NLM procedure calls from
//! connection read tasks, forwards them to the [`Nlm`] service, and sends
//! the serialized reply back to the appropriate write task.

use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::debug;

use crate::nlm::Nlm;
use crate::task::{ProcReply, ProcResult};
use crate::{
    nlm::NlmRes,
    parser::{NlmArgWrapper, NlmArguments},
};

/// Command sent to [`NlmTask`] from connection read tasks.
pub struct NlmCommand {
    /// Channel used to pass the result to write task.
    pub result_tx: UnboundedSender<ProcReply>,
    /// Parsed NLM procedure arguments together with the RPC header.
    pub args: NlmArgWrapper,
}

/// Background task that processes NLMv4 commands sequentially.
///
/// Receives [`NlmCommand`] values from all active connections, dispatches
/// them to the shared [`Nlm`] service, and sends the resulting
/// [`ProcResult::Nlm4`] back to the originating connection for
/// serialization and transmission.
pub struct NlmTask<N>
where
    N: Nlm + Send + Sync + 'static,
{
    /// Shared NLM service implementation.
    nlm_service: Arc<N>,

    /// Channel for commands from client connection tasks.
    receiver: UnboundedReceiver<NlmCommand>,
}

impl<N> NlmTask<N>
where
    N: Nlm + Send + Sync + 'static,
{
    /// Creates a new [`NlmTask`] and returns the task together with a
    /// sender handle that connection tasks use to submit commands.
    pub fn new(nlm_service: Arc<N>) -> (Self, UnboundedSender<NlmCommand>) {
        let (sender, receiver) = mpsc::unbounded_channel::<NlmCommand>();

        let task = Self { nlm_service, receiver };

        (task, sender)
    }

    /// Spawns the [`NlmTask`] on the current Tokio runtime.
    ///
    /// The task processes NLM commands received from read tasks and
    /// returns results to write tasks.
    ///
    /// # Panics
    ///
    /// If called outside a Tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    /// Main event loop: waits for commands, dispatches to the NLM service,
    /// and sends replies back.
    async fn run(self) {
        let nlm_service = self.nlm_service;
        let mut receiver = self.receiver;

        while let Some(command) = receiver.recv().await {
            let NlmCommand { result_tx, args } = command;
            let NlmArgWrapper { header, proc } = args;
            debug!(xid = header.xid, "nlm task: command received");

            let nlm_result = match *proc {
                NlmArguments::Null => NlmRes::Null,
                NlmArguments::Lock(nlm4_lock_args) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM");
                    let res = nlm_service.lock(nlm4_lock_args).await;
                    NlmRes::Lock(Box::new(res))
                }
                NlmArguments::Unlock(nlm4_unlock_args) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM");
                    let res = nlm_service.unlock(nlm4_unlock_args).await;
                    NlmRes::Unlock(res)
                }
                NlmArguments::Test(nlm4_test_args) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM");
                    let res = nlm_service.test(nlm4_test_args).await;
                    NlmRes::Test(Box::new(res))
                }
                NlmArguments::Cancel(nlm4_cancel_args) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM");
                    let res = nlm_service.cancel(nlm4_cancel_args).await;
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
