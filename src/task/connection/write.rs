use std::sync::Arc;

use async_channel::Receiver;
use async_trait::async_trait;
use tokio_uring::net::TcpStream;
use tracing::error;

use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::serializer::server::serialize_struct::WriteSink;
use crate::task::ProcReply;

struct UringWriteStream {
    socket: Arc<TcpStream>,
}

impl UringWriteStream {
    fn new(socket: Arc<TcpStream>) -> Self {
        Self { socket }
    }
}

#[async_trait(?Send)]
impl WriteSink for UringWriteStream {
    async fn write_all_bytes(&mut self, buf: &[u8]) -> std::io::Result<()> {
        let (result, _) = self.socket.write_all(buf.to_vec()).await;
        result
    }
}

/// Writes [`super::super::global::vfs::VfsPool`] responses to a network connection.
pub struct WriteTask {
    socket: Arc<TcpStream>,
    result_receiver: Receiver<ProcReply>,
}

impl WriteTask {
    /// Creates new instance of [`WriteTask`]
    pub fn new(socket: Arc<TcpStream>, result_receiver: Receiver<ProcReply>) -> Self {
        Self { socket, result_receiver }
    }

    /// Spawns a [`WriteTask`]  that writes command results to a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio-uring runtime context.
    pub fn spawn(self) {
        tokio_uring::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let result_receiver = self.result_receiver;
        let mut serializer = serializer::server::serialize_struct::Serializer::new(
            UringWriteStream::new(self.socket),
        );

        while let Ok(reply) = result_receiver.recv().await {
            // TODO: <https://github.com/RMamonts/nfs-mamont/issues/143>
            // Use proper authentication verifier instead of None
            let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };

            match serializer.form_reply(reply, verifier).await {
                Ok(_) => {
                    // Reply successfully written to socket
                }
                Err(e) => {
                    error!(error=%e, "write task: failed to serialize/send reply");
                    // TODO: Consider closing connection or continuing based on error type
                    // For now, continue processing other replies
                }
            };
        }
    }
}
