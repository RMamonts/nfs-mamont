use monoio::net::tcp::TcpOwnedWriteHalf;
use async_channel;
use tracing::error;

use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::task::ProcReply;

pub struct WriteTask {
    writehalf: TcpOwnedWriteHalf,
    result_receiver: async_channel::Receiver<ProcReply>,
}

impl WriteTask {
    pub fn new(writehalf: TcpOwnedWriteHalf, result_receiver: async_channel::Receiver<ProcReply>) -> Self {
        Self { writehalf, result_receiver }
    }

    pub fn spawn(self) {
        monoio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let result_receiver = self.result_receiver;
        let mut serializer = serializer::server::serialize_struct::Serializer::new(self.writehalf);

        while let Ok(reply) = result_receiver.recv().await {
            let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };

            match serializer.form_reply(reply, verifier).await {
                Ok(_) => {}
                Err(e) => {
                    error!(error=%e, "write task: failed to serialize/send reply");
                }
            };
        }
    }
}
