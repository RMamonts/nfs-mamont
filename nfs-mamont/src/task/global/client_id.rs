//! Server-owned client registration actor.
//!
//! One task serializes both protocol registries and owns their shared clientid
//! generator. Each command carries a one-shot reply channel. Dropping the reply
//! receiver does not cancel the registration or stop the actor. Dropping all
//! command senders lets the actor drain queued commands and exit.
//!
//! Construct and spawn this task once when starting a server. It is not yet
//! connected to the server accept loop or RPC procedure handlers.
//!
//! ```no_run
//! use nfs_mamont::{auth::Credential, client_id::*};
//! use tokio::sync::oneshot;
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
//! let (task, sender) = ClientIdTask::new();
//! let running = task.spawn();
//! let mut generations = ConnectionGenerationGenerator::new();
//! let connection = ConnectionInfo::new(
//!     "192.0.2.1:40000".parse().unwrap(), generations.allocate(),
//! );
//! let (result_sender, result_receiver) = oneshot::channel();
//! sender.send(ClientIdCommand::RegisterUnchecked {
//!     connection, credentials: Credential::None, result_sender,
//! }).await.unwrap();
//! let clientid = result_receiver.await.unwrap().unwrap();
//! drop(sender);
//! running.await.unwrap();
//! # }
//! ```

use async_channel::{Receiver, Sender};
use tokio::{sync::oneshot, task::JoinHandle};

use crate::{
    auth::Credential,
    client_id::{
        ClientId, ClientIdGenerator, ClientIdManager, ConnectionInfo, Nfs3ClientIdManager,
        PendingClient, Registration, Result,
    },
};

/// Sender cloned by callers of the client registration actor.
pub type ClientIdSender = Sender<ClientIdCommand>;
/// Receiving end of the actor's command channel.
pub type ClientIdReceiver = Receiver<ClientIdCommand>;
/// One-shot result sender included in a registration command.
pub type ClientIdReplySender<T> = oneshot::Sender<Result<T>>;
/// One-shot result receiver retained by the caller.
pub type ClientIdReplyReceiver<T> = oneshot::Receiver<Result<T>>;

/// Commands handled by the server's client registration actor.
pub enum ClientIdCommand {
    /// NFSv4 registration; the returned pair must subsequently be confirmed.
    Register {
        registration: Registration,
        credentials: Credential,
        result_sender: ClientIdReplySender<PendingClient>,
    },
    /// Confirms a previously returned NFSv4 registration pair.
    Confirm { pair: PendingClient, credentials: Credential, result_sender: ClientIdReplySender<()> },
    /// Immediately registers an NFSv3 peer, including credential collision checks.
    RegisterUnchecked {
        connection: ConnectionInfo,
        credentials: Credential,
        result_sender: ClientIdReplySender<ClientId>,
    },
}

/// Owns both client registries and their common ID allocator.
pub struct ClientIdTask {
    nfsv4: ClientIdManager,
    nfsv3: Nfs3ClientIdManager,
    receiver: ClientIdReceiver,
}

impl ClientIdTask {
    /// Creates an unstarted actor and its command sender.
    pub fn new() -> (Self, ClientIdSender) {
        let (sender, receiver) = async_channel::unbounded();
        let ids = ClientIdGenerator::new();
        let task = Self {
            nfsv4: ClientIdManager::new(ids.clone()),
            nfsv3: Nfs3ClientIdManager::new(ids),
            receiver,
        };
        (task, sender)
    }

    /// Spawns the actor on the current Tokio runtime and returns its completion
    /// handle. Panics if called outside a Tokio runtime.
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(self.run())
    }

    async fn run(mut self) {
        while let Ok(command) = self.receiver.recv().await {
            match command {
                ClientIdCommand::Register { registration, credentials, result_sender } => {
                    let result = self.nfsv4.register(registration, &credentials);
                    let _ = result_sender.send(result);
                }
                ClientIdCommand::Confirm { pair, credentials, result_sender } => {
                    let result = self.nfsv4.confirm(pair, &credentials);
                    let _ = result_sender.send(result);
                }
                ClientIdCommand::RegisterUnchecked { connection, credentials, result_sender } => {
                    let result = self.nfsv3.register(connection, &credentials);
                    let _ = result_sender.send(result);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        auth::AuthSysParams,
        client_id::{
            Callback, ClientIdentity, ClientVerifier, ConnectionGenerationGenerator, Error, NetId,
        },
    };

    fn registration(verifier: u8, ident: u32) -> Registration {
        Registration {
            identity: ClientIdentity::new(b"client".to_vec()).unwrap(),
            verifier: ClientVerifier::new([verifier; 8]),
            callback: Callback {
                netid: NetId::new("tcp".into()),
                address: "192.0.2.1:2049".parse().unwrap(),
                program: 123,
                ident,
            },
        }
    }

    async fn register(sender: &ClientIdSender, verifier: u8, ident: u32) -> PendingClient {
        let (result_sender, receiver) = oneshot::channel();
        sender
            .send(ClientIdCommand::Register {
                registration: registration(verifier, ident),
                credentials: Credential::None,
                result_sender,
            })
            .await
            .unwrap();
        receiver.await.unwrap().unwrap()
    }

    async fn confirm(
        sender: &ClientIdSender,
        pair: PendingClient,
        credentials: Credential,
    ) -> Result<()> {
        let (result_sender, receiver) = oneshot::channel();
        sender.send(ClientIdCommand::Confirm { pair, credentials, result_sender }).await.unwrap();
        receiver.await.unwrap()
    }

    async fn unchecked(
        sender: &ClientIdSender,
        connection: ConnectionInfo,
        credentials: Credential,
    ) -> Result<ClientId> {
        let (result_sender, receiver) = oneshot::channel();
        sender
            .send(ClientIdCommand::RegisterUnchecked { connection, credentials, result_sender })
            .await
            .unwrap();
        receiver.await.unwrap()
    }

    fn foreign() -> Credential {
        Credential::Sys(AuthSysParams {
            stamp: 1,
            machine_name: "host".into(),
            uid: 100,
            gid: 10,
            gids: vec![],
        })
    }

    #[tokio::test]
    async fn protocols_share_ids_and_retries_preserve_them() {
        let (task, sender) = ClientIdTask::new();
        let running = task.spawn();
        let mut generations = ConnectionGenerationGenerator::new();
        let connection =
            ConnectionInfo::new("192.0.2.1:40000".parse().unwrap(), generations.allocate());
        let first = register(&sender, 1, 1).await;
        let peer = unchecked(&sender, connection, Credential::None).await.unwrap();
        assert_ne!(first.clientid, peer);
        assert_eq!(register(&sender, 1, 1).await, first);
        assert_eq!(confirm(&sender, first, Credential::None).await, Ok(()));
        let update = register(&sender, 1, 2).await;
        assert_eq!(update.clientid, first.clientid);
        assert_ne!(update.confirmation, first.confirmation);
        assert_eq!(confirm(&sender, update, Credential::None).await, Ok(()));
        let reboot = register(&sender, 2, 1).await;
        assert_ne!(reboot.clientid, first.clientid);
        assert_ne!(reboot.clientid, peer);
        assert_eq!(reboot.clientid.into_inner(), peer.into_inner() + 1);
        assert_eq!(unchecked(&sender, connection, Credential::None).await, Ok(peer));
        assert_eq!(confirm(&sender, reboot, Credential::None).await, Ok(()));
        drop(sender);
        running.await.unwrap();
    }

    #[tokio::test]
    async fn errors_and_abandoned_replies_do_not_stop_actor() {
        let (task, sender) = ClientIdTask::new();
        let running = task.spawn();
        let (result_sender, receiver) = oneshot::channel();
        drop(receiver);
        sender
            .send(ClientIdCommand::Register {
                registration: registration(1, 1),
                credentials: Credential::None,
                result_sender,
            })
            .await
            .unwrap();
        let pair = register(&sender, 1, 1).await;
        assert_eq!(confirm(&sender, pair, foreign()).await, Err(Error::IdentityInUse));
        assert_eq!(confirm(&sender, pair, Credential::None).await, Ok(()));
        let mut generations = ConnectionGenerationGenerator::new();
        let connection =
            ConnectionInfo::new("192.0.2.1:40000".parse().unwrap(), generations.allocate());
        let peer = unchecked(&sender, connection, Credential::None).await.unwrap();
        assert_eq!(unchecked(&sender, connection, foreign()).await, Err(Error::IdentityInUse));
        assert_eq!(unchecked(&sender, connection, Credential::None).await, Ok(peer));
        drop(sender);
        running.await.unwrap();
    }
}
