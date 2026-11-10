use std::time::Duration;

use tokio::net::tcp::OwnedWriteHalf;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::{error, warn};

use crate::allocator::Buffer;
use crate::client_id::ConnectionInfo;
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer;
use crate::task::connection::limit::InFlightReply;

/// Writes [`super::super::global::vfs::VfsTask`] responses to a network connection.
pub struct WriteTask<B: Buffer> {
    writehalf: OwnedWriteHalf,
    result_receiver: async_channel::Receiver<InFlightReply<B>>,
    connection_info: ConnectionInfo,
    /// How long one reply may wait for the socket, `None` for ever.
    write_timeout: Option<Duration>,
}

impl<B: Buffer> WriteTask<B> {
    /// Creates new instance of [`WriteTask`]
    pub fn new(
        writehalf: OwnedWriteHalf,
        result_receiver: async_channel::Receiver<InFlightReply<B>>,
        connection_info: ConnectionInfo,
        write_timeout: Option<Duration>,
    ) -> Self {
        Self { writehalf, result_receiver, connection_info, write_timeout }
    }

    /// Spawns a [`WriteTask`] that writes command results to a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) -> JoinHandle<()>
    where
        B: 'static,
    {
        tokio::spawn(async move { self.run().await })
    }

    async fn run(self) {
        let result_receiver = self.result_receiver;
        let mut serializer = serializer::server::serialize_struct::Serializer::new(self.writehalf);

        while let Ok(InFlightReply { reply, token }) = result_receiver.recv().await {
            // TODO: <https://github.com/RMamonts/nfs-mamont/issues/143>
            // Use proper authentication verifier instead of None
            let verifier = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };

            let write = serializer.form_reply(reply, verifier);
            let written = match self.write_timeout {
                Some(limit) => timeout(limit, write).await,
                None => Ok(write.await),
            };
            let Ok(written) = written else {
                warn!(
                    client=%self.connection_info.peer_addr,
                    timeout=?self.write_timeout,
                    "write task: the client does not take its replies, closing the connection"
                );
                return;
            };
            match written {
                Ok(_) => {
                    // Reply successfully written to socket
                }
                Err(e) => {
                    error!(client=%self.connection_info.peer_addr, error=%e, "write task: failed to serialize/send reply");
                    // TODO: Consider closing connection or continuing based on error type
                    // For now, continue processing other replies
                }
            };

            // Freeing the slot before the reply is written would let replies pile up.
            drop(token);
        }
    }
}
