//! NLMv4 task dispatcher.
//!
//! Runs a background task that receives parsed NLM procedure calls from
//! connection read tasks, forwards them to the [`Nlm`] service, and sends
//! the serialized reply back to the appropriate write task.

use async_channel::{Receiver, Sender};
use std::sync::Arc;
use tracing::{debug, error};

use crate::allocator::Buffer;
use crate::nlm::Nlm;
use crate::task::{ProcReply, ProcResult};
use crate::{
    nlm::NlmRes,
    parser::{NlmArgWrapper, NlmArguments},
};

/// The structure that expects NlmTask. Contains arguments and channels for responses and calls.
pub struct NlmCommand<B: Buffer> {
    /// Channel used to pass the result to write task.
    pub result_sender: Sender<ProcReply<B>>,
    /// Placeholder for NLM procedure args.
    pub args: NlmArgWrapper,
}

pub struct NlmTask<B, N>
where
    B: Buffer + 'static,
    N: Nlm + Send + Sync + 'static,
{
    /// Shared NLM service implementation.
    nlm_service: Arc<N>,

    /// Channel for commands from client connection tasks
    receiver: Receiver<NlmCommand<B>>,
}

impl<B, N> NlmTask<B, N>
where
    B: Buffer + 'static,
    N: Nlm + Send + Sync + 'static,
{
    /// Creates new instance of [`NlmTask`]
    pub fn new(nlm_service: Arc<N>) -> (Self, Sender<NlmCommand<B>>) {
        let (sender, receiver) = async_channel::unbounded::<NlmCommand<B>>();

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
        let receiver = self.receiver;

        while let Ok(command) = receiver.recv().await {
            let NlmCommand { result_sender, args } = command;
            let xid = args.header.xid;
            let nlm_result = process_message(args, Arc::clone(&nlm_service)).await;
            if let Some(result) = nlm_result {
                send_reply(result_sender, result, xid).await;
            }
        }
    }
}

async fn send_reply<B: Buffer>(result_sender: Sender<ProcReply<B>>, nlm_result: NlmRes, xid: u32) {
    // TODO:
    // - some logs when occurred error
    // - or retry with fail
    // * but don't stop task
    if let Err(e) = result_sender
        .send(ProcReply { xid, proc_result: Ok(ProcResult::Nlm4(Box::new(nlm_result))) })
        .await
    {
        error!(xid = xid, "nlm task: failed to send reply: {}", e);
    } else {
        debug!(xid = xid, "nlm task: reply queued");
    }
}

async fn process_message<N: Nlm + Send + Sync + 'static>(
    args: NlmArgWrapper,
    nlm_service: Arc<N>,
) -> Option<NlmRes> {
    let NlmArgWrapper { header, proc } = args;
    debug!(xid = header.xid, "nlm task: command received");

    match *proc {
        NlmArguments::Null => Some(NlmRes::Null),
        NlmArguments::Lock(nlm4_lock_args) => {
            debug!(xid = header.xid, "nlm task: proc=NLM LOCK");
            let res = nlm_service.lock(nlm4_lock_args, &header.cred).await;
            Some(NlmRes::Lock(res))
        }
        NlmArguments::Unlock(nlm4_unlock_args) => {
            debug!(xid = header.xid, "nlm task: proc=NLM UNLOCK");
            let res = nlm_service.unlock(nlm4_unlock_args, &header.cred).await;
            Some(NlmRes::Unlock(res))
        }
        NlmArguments::Test(nlm4_test_args) => {
            debug!(xid = header.xid, "nlm task: proc=NLM TEST");
            let res = nlm_service.test(nlm4_test_args, &header.cred).await;
            Some(NlmRes::Test(Box::new(res)))
        }
        NlmArguments::Cancel(nlm4_cancel_args) => {
            debug!(xid = header.xid, "nlm task: proc=NLM CANCEL");
            let res = nlm_service.cancel(nlm4_cancel_args, &header.cred).await;
            Some(NlmRes::Cancel(res))
        }
    }
}
