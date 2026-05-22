use async_channel;
use tracing::debug;

use crate::task::{ProcReply, ProcResult};
use crate::{
    nlm::NlmRes,
    parser::{NlmArgWrapper, NlmArguments},
};

pub struct NlmCommand {
    pub result_tx: async_channel::Sender<ProcReply>,
    pub args: NlmArgWrapper,
}

pub struct NlmTask {
    receiver: async_channel::Receiver<NlmCommand>,
}

impl NlmTask {
    pub fn new() -> (Self, async_channel::Sender<NlmCommand>) {
        let (sender, receiver) = async_channel::unbounded::<NlmCommand>();

        let task = Self { receiver };

        (task, sender)
    }

    pub fn spawn(self) {
        monoio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let receiver = self.receiver;

        while let Ok(command) = receiver.recv().await {
            let NlmCommand { result_tx, args } = command;
            let NlmArgWrapper { header, proc } = args;
            debug!(xid = header.xid, "nlm task: command received");

            let nlm_result = match *proc {
                NlmArguments::Null => NlmRes::Null,
                _ => todo!(),
            };

            let _ = result_tx.send(ProcReply {
                xid: header.xid,
                proc_result: Ok(ProcResult::Nlm4(Box::new(nlm_result))),
            }).await;
            debug!(xid = header.xid, "nlm task: reply queued");
        }
    }
}
