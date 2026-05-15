use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::debug;

use crate::task::{ProcReply, ProcResult};
use crate::{
    nlm::NlmRes,
    parser::{NlmArgWrapper, NlmArguments},
};

pub struct NlmCommand {
    /// Channel used to pass the result to write task.
    pub result_tx: UnboundedSender<ProcReply>,
    /// Placeholder for NLM procedure args.
    pub args: NlmArgWrapper,
}

pub struct NlmTask {
    // Channel for commands from client connection tasks
    receiver: UnboundedReceiver<NlmCommand>,
}

impl NlmTask {
    /// Creates new instance of [`NlmTask`]
    pub fn new() -> (Self, UnboundedSender<NlmCommand>) {
        let (sender, receiver) = mpsc::unbounded_channel::<NlmCommand>();

        let task = Self { receiver };

        (task, sender)
    }

    /// Spawns a [`NlmTask`]  that processes nlm commands received from
    /// `ReadTask` and returns results to `WriteTask`.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        monoio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let mut receiver = self.receiver;

        while let Some(command) = receiver.recv().await {
            let NlmCommand { result_tx, args } = command;
            let NlmArgWrapper { header, proc } = args;
            debug!(xid = header.xid, "nlm task: command received");

            let nlm_result = match *proc {
                NlmArguments::Null => NlmRes::Null,
                _ => todo!(),
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
