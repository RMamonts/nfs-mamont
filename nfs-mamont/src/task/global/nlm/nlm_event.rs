use crate::nlm::procedures::test::Nlm4TestArgs;
use crate::nlm::NlmCallbackReply;
use crate::task::{ProcCall, ProcMessage};
use async_channel::Sender;

/// The abstraction necessary to encapsulate the host channel in WriteTask.
/// The channel is required to identify the client.
#[derive(Clone)]
pub struct NlmEventHandler {
    /// The channel for sending an event.
    message_sender: Sender<ProcCall>,
}

impl NlmEventHandler {
    pub fn new(message_sender: Sender<ProcCall>) -> Self {
        Self { message_sender }
    }

    /// Call this method to make an RPC call as specified in the RFC1813.
    pub async fn granted(&self, test_args: Nlm4TestArgs) {
        let callback_reply = NlmCallbackReply::Granted(test_args);
        let proc_message = ProcMessage::Nlm4(callback_reply);
        let call = ProcCall::new(proc_message);
        if let Err(e) = self.message_sender.send(call).await {
            tracing::warn!("failed to send grant callback: {}", e);
        }
    }
}
