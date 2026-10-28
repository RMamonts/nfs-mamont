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
use crate::task::connection::limit::Token;
use crate::task::{InFlightReply, ProcReply, ProcResult};
use crate::{
    nlm::NlmRes,
    parser::{NlmArgWrapper, NlmArguments},
};

/// The structure that expects NlmTask. Contains arguments and channels for responses and calls.
pub struct NlmCommand<B: Buffer> {
    /// Channel used to pass the result to write task.
    pub result_sender: Sender<InFlightReply<B>>,
    /// Placeholder for NLM procedure args.
    pub args: NlmArgWrapper,
    /// In-flight slot of the request, returned by the write task after the reply is
    /// written, see [`crate::task::connection`].
    pub token: Token,
}

/// The main task is to obtain/send nlm procedures.
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
            let NlmCommand { result_sender, args, token } = command;
            let xid = args.header.xid;
            let nlm_result = process_message(args, Arc::clone(&nlm_service)).await;
            send_reply(result_sender, nlm_result, xid, token).await;
        }
    }
}

async fn send_reply<B: Buffer>(
    result_sender: Sender<InFlightReply<B>>,
    nlm_result: NlmRes,
    xid: u32,
    token: Token,
) {
    // TODO:
    // - some logs when occurred error
    // - or retry with fail
    // * but don't stop task
    let reply = ProcReply { xid, proc_result: Ok(ProcResult::Nlm4(Box::new(nlm_result))) };
    if let Err(e) = result_sender.send(InFlightReply { reply, token }).await {
        error!(xid = xid, "nlm task: failed to send reply: {}", e);
    } else {
        debug!(xid = xid, "nlm task: reply queued");
    }
}

/// Runs the procedure against the NLM service; every procedure yields a reply, which
/// carries the request's in-flight token back to the write task.
async fn process_message<N: Nlm + Send + Sync + 'static>(
    args: NlmArgWrapper,
    nlm_service: Arc<N>,
) -> NlmRes {
    let NlmArgWrapper { header, proc } = args;
    debug!(xid = header.xid, "nlm task: command received");

    match *proc {
        NlmArguments::Null => NlmRes::Null,
        NlmArguments::Lock(nlm4_lock_args) => {
            debug!(xid = header.xid, "nlm task: proc=NLM LOCK");
            let res = nlm_service.lock(nlm4_lock_args, &header.cred).await;
            NlmRes::Lock(res)
        }
        NlmArguments::Unlock(nlm4_unlock_args) => {
            debug!(xid = header.xid, "nlm task: proc=NLM UNLOCK");
            let res = nlm_service.unlock(nlm4_unlock_args, &header.cred).await;
            NlmRes::Unlock(res)
        }
        NlmArguments::Test(nlm4_test_args) => {
            debug!(xid = header.xid, "nlm task: proc=NLM TEST");
            let res = nlm_service.test(nlm4_test_args, &header.cred).await;
            NlmRes::Test(Box::new(res))
        }
        NlmArguments::Cancel(nlm4_cancel_args) => {
            debug!(xid = header.xid, "nlm task: proc=NLM CANCEL");
            let res = nlm_service.cancel(nlm4_cancel_args, &header.cred).await;
            NlmRes::Cancel(res)
        }
    }
}
