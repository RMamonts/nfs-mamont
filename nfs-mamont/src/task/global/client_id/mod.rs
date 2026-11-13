//! Server-owned client registration actor.
//!
//! One task serializes both protocol registries and owns their shared clientid
//! generator. Each command carries a one-shot reply channel. Dropping the reply
//! receiver does not cancel the registration or stop the actor. Dropping all
//! command senders lets the actor drain queued commands and exit.
//!
//! Registries start empty and fill as registration commands arrive. The same
//! task drives a DelayQueue for active leases and publishes expired client IDs
//! to a separate channel. Its consumer owns cleanup of locks and callbacks.
//! This task is not yet connected to the accept loop or RPC procedure handlers.
//!
//! ```no_run
//! use nfs_mamont::{auth::Credential, client_id::*};
//! use tokio::sync::oneshot;
//! use std::time::Duration;
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
//! let (task, sender, expired) = ClientIdTask::new(Duration::from_secs(60));
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

mod leases;
mod registries;

use std::time::Duration;

use async_channel::{Receiver, Sender};
use tokio::{sync::oneshot, task::JoinHandle};

use crate::auth::Credential;
use crate::client_id::{ClientId, ConnectionInfo, PendingClient, Registration, Result};

use self::leases::{ClientLeases, RemovedLease};
use self::registries::ClientRegistries;

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
    /// Extends an active lease after authenticated client activity.
    Renew { clientid: ClientId, credentials: Credential, result_sender: ClientIdReplySender<()> },
    /// Removes a client and cancels its timer. Returns the removed active ID for
    /// state cleanup, or `None` for an unconfirmed or unknown client.
    Invalidate { clientid: ClientId, result_sender: ClientIdReplySender<Option<ClientId>> },
}

/// Owns both client registries and their common ID allocator.
pub struct ClientIdTask {
    clients: ClientRegistries,
    receiver: ClientIdReceiver,
    leases: ClientLeases,
    expired_sender: Sender<ClientId>,
}

impl ClientIdTask {
    /// Creates an empty, unstarted actor, its command sender and an unbounded
    /// expiration receiver. Leases start on NFSv4 confirmation or NFSv3
    /// registration; unconfirmed records have no lease. Expiration invalidates
    /// the registration before sending its ID. Dropping the expiration receiver
    /// does not stop command processing. Dropping all command senders drains
    /// queued commands and stops the actor without waiting for active leases.
    ///
    /// A zero duration makes leases expire immediately. Timer operations require
    /// a Tokio runtime with time enabled and a duration supported by Tokio timers.
    pub fn new(lease_duration: Duration) -> (Self, ClientIdSender, Receiver<ClientId>) {
        let (sender, receiver) = async_channel::unbounded();
        let (expired_sender, expired_receiver) = async_channel::unbounded();
        let task = Self {
            clients: ClientRegistries::new(),
            receiver,
            leases: ClientLeases::new(lease_duration),
            expired_sender,
        };
        (task, sender, expired_receiver)
    }

    /// Spawns the actor on the current Tokio runtime and returns its completion
    /// handle. Panics if called outside a Tokio runtime.
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(self.run())
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                command = self.receiver.recv() => {
                    let Ok(command) = command else { break };
                    self.handle(command);
                }
                Some(lease) = self.leases.expired(), if !self.leases.is_empty() => {
                    self.expire(lease);
                }
            }
        }
    }

    fn handle(&mut self, command: ClientIdCommand) {
        match command {
            ClientIdCommand::Register { registration, credentials, result_sender } => {
                if let Some(clientid) = self.clients.lookup(&registration.identity) {
                    self.expire_if_due(clientid);
                }
                let result = self.clients.register(registration, &credentials);
                send_reply(result_sender, result);
            }
            ClientIdCommand::Confirm { pair, credentials, result_sender } => {
                self.expire_if_due(pair.clientid);
                let result = self.clients.confirm(pair, &credentials).and_then(|confirmation| {
                    // Confirmation DRC may replay success after invalidation.
                    // Preserve the reply, but never revive an expired client.
                    if let Some(old) = confirmation.retired {
                        // The manager already removed the old registration.
                        if let Some(lease) = self.leases.remove(old) {
                            if lease.is_expired() {
                                self.notify_expired(lease.clientid);
                            }
                        }
                    }
                    if let Some(clientid) = confirmation.clientid {
                        self.leases.register(clientid, credentials)?;
                    }
                    Ok(())
                });
                send_reply(result_sender, result);
            }
            ClientIdCommand::RegisterUnchecked { connection, credentials, result_sender } => {
                if let Some(clientid) = self.clients.lookup_unchecked(connection) {
                    self.expire_if_due(clientid);
                }
                let result = self.clients.register_unchecked(connection, &credentials).and_then(
                    |clientid| {
                        self.leases.register(clientid, credentials)?;
                        Ok(clientid)
                    },
                );
                send_reply(result_sender, result);
            }
            ClientIdCommand::Renew { clientid, credentials, result_sender } => {
                let result = self.renew(clientid, &credentials);
                send_reply(result_sender, result);
            }
            ClientIdCommand::Invalidate { clientid, result_sender } => {
                let removed = self.invalidate(clientid);
                send_reply(result_sender, Ok(removed));
            }
        }
    }

    #[must_use]
    fn invalidate(&mut self, clientid: ClientId) -> Option<ClientId> {
        let removed = self.clients.invalidate(clientid);
        if let (Some(lease), None) = (self.leases.remove(clientid), removed) {
            tracing::warn!(
                clientid = lease.clientid.into_inner(),
                "invalidated client registration was already removed"
            );
        }
        removed
    }

    fn expire(&mut self, lease: RemovedLease) {
        let clientid = lease.clientid;
        if self.clients.invalidate(clientid).is_none() {
            tracing::warn!(
                clientid = clientid.into_inner(),
                "expired client registration was already removed"
            );
        }
        // Locks may still need cleanup even if the registration is already gone.
        self.notify_expired(clientid);
    }

    fn notify_expired(&self, clientid: ClientId) {
        // The output channel is unbounded, so cleanup cannot stall this actor.
        if let Err(error) = self.expired_sender.try_send(clientid) {
            tracing::warn!(clientid = clientid.into_inner(), %error, "failed to deliver expired client ID");
        }
    }

    fn expire_if_due(&mut self, clientid: ClientId) {
        if let Some(lease) = self.leases.remove_expired(clientid) {
            self.expire(lease);
        }
    }

    fn renew(&mut self, clientid: ClientId, credentials: &Credential) -> Result<()> {
        self.expire_if_due(clientid);
        self.leases.renew(clientid, credentials)
    }
}

// TODO: define behaviour in case of such error
fn send_reply<T>(sender: ClientIdReplySender<T>, result: Result<T>) {
    if sender.send(result).is_err() {
        tracing::debug!("client ID reply receiver dropped");
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
        let (task, sender, _expired) = ClientIdTask::new(Duration::from_secs(60));
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
        let (task, sender, _expired) = ClientIdTask::new(Duration::from_secs(60));
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

    const LEASE: Duration = Duration::from_secs(10);

    fn connection() -> ConnectionInfo {
        ConnectionInfo::new(
            "192.0.2.1:40000".parse().unwrap(),
            ConnectionGenerationGenerator::new().allocate(),
        )
    }

    async fn renew(
        sender: &ClientIdSender,
        clientid: ClientId,
        credentials: Credential,
    ) -> Result<()> {
        let (result_sender, receiver) = oneshot::channel();
        sender.send(ClientIdCommand::Renew { clientid, credentials, result_sender }).await.unwrap();
        receiver.await.unwrap()
    }

    async fn invalidate(sender: &ClientIdSender, clientid: ClientId) -> Option<ClientId> {
        let (result_sender, receiver) = oneshot::channel();
        sender.send(ClientIdCommand::Invalidate { clientid, result_sender }).await.unwrap();
        receiver.await.unwrap().unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn leases_expire_once_for_both_protocols_and_empty_queue_accepts_new_clients() {
        let (task, sender, expired) = ClientIdTask::new(LEASE);
        let running = task.spawn();
        tokio::task::yield_now().await;
        let pending = register(&sender, 1, 1).await;
        assert_eq!(confirm(&sender, pending, Credential::None).await, Ok(()));
        let peer = unchecked(&sender, connection(), Credential::None).await.unwrap();
        assert_ne!(pending.clientid, peer);
        tokio::time::advance(LEASE + Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        let mut ids = [expired.try_recv().unwrap(), expired.try_recv().unwrap()];
        ids.sort_by_key(|id| id.into_inner());
        let mut expected = [pending.clientid, peer];
        expected.sort_by_key(|id| id.into_inner());
        assert_eq!(ids, expected);
        assert!(expired.try_recv().is_err());
        assert_eq!(renew(&sender, peer, Credential::None).await, Err(Error::StaleClientId));
        // The registration manager preserves cached replies, but a successful
        // replay must not recreate a lease for a removed registration.
        assert_eq!(confirm(&sender, pending, Credential::None).await, Ok(()));
        assert_eq!(
            renew(&sender, pending.clientid, Credential::None).await,
            Err(Error::StaleClientId)
        );
        let next = unchecked(&sender, connection(), Credential::None).await.unwrap();
        assert_ne!(next, peer);
        tokio::time::advance(LEASE + Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(expired.try_recv().unwrap(), next);
        assert!(expired.try_recv().is_err());
        drop(sender);
        running.await.unwrap();
        assert!(expired.recv().await.is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn renewal_resets_timer_and_bad_credentials_do_not_extend_lease() {
        let (task, sender, expired) = ClientIdTask::new(LEASE);
        let running = task.spawn();
        let peer = unchecked(&sender, connection(), Credential::None).await.unwrap();
        tokio::time::advance(Duration::from_secs(6)).await;
        assert_eq!(renew(&sender, peer, Credential::None).await, Ok(()));
        tokio::time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;
        assert!(expired.try_recv().is_err());
        assert_eq!(renew(&sender, peer, foreign()).await, Err(Error::IdentityInUse));
        assert_eq!(unchecked(&sender, connection(), foreign()).await, Err(Error::IdentityInUse));
        tokio::time::advance(Duration::from_secs(6)).await;
        tokio::task::yield_now().await;
        assert_eq!(expired.try_recv().unwrap(), peer);
        assert!(expired.try_recv().is_err());
        drop(sender);
        running.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn overdue_renewal_is_rejected_even_if_command_is_handled_before_timer() {
        let (mut task, sender, expired) = ClientIdTask::new(LEASE);
        let (result_sender, receiver) = oneshot::channel();
        task.handle(ClientIdCommand::RegisterUnchecked {
            connection: connection(),
            credentials: Credential::None,
            result_sender,
        });
        let peer = receiver.await.unwrap().unwrap();
        tokio::time::advance(LEASE + Duration::from_millis(1)).await;
        let (result_sender, receiver) = oneshot::channel();
        task.handle(ClientIdCommand::Renew {
            clientid: peer,
            credentials: Credential::None,
            result_sender,
        });
        assert_eq!(receiver.await.unwrap(), Err(Error::StaleClientId));
        assert_eq!(expired.try_recv().unwrap(), peer);
        assert!(task.leases.is_empty());
        drop(sender);
        task.run().await;
    }

    #[tokio::test(start_paused = true)]
    async fn confirmation_reports_overdue_previous_client_before_replacement() {
        let (mut task, _sender, expired) = ClientIdTask::new(LEASE);
        let first = task.clients.register(registration(1, 1), &Credential::None).unwrap();
        task.clients.confirm(first, &Credential::None).unwrap();
        task.leases.register(first.clientid, Credential::None).unwrap();
        let reboot = task.clients.register(registration(2, 1), &Credential::None).unwrap();
        tokio::time::advance(LEASE + Duration::from_millis(1)).await;
        let (result_sender, receiver) = oneshot::channel();
        task.handle(ClientIdCommand::Confirm {
            pair: reboot,
            credentials: Credential::None,
            result_sender,
        });
        assert_eq!(receiver.await.unwrap(), Ok(()));
        assert_eq!(expired.try_recv().unwrap(), first.clientid);
        assert_eq!(task.leases.renew(first.clientid, &Credential::None), Err(Error::StaleClientId));
        assert_eq!(task.leases.renew(reboot.clientid, &Credential::None), Ok(()));
    }

    #[tokio::test(start_paused = true)]
    async fn callback_confirmation_renews_and_reboot_cancels_previous_timer() {
        let (task, sender, expired) = ClientIdTask::new(LEASE);
        let running = task.spawn();
        let first = register(&sender, 1, 1).await;
        assert_eq!(confirm(&sender, first, Credential::None).await, Ok(()));
        tokio::time::advance(Duration::from_secs(6)).await;
        let update = register(&sender, 1, 2).await;
        assert_eq!(update.clientid, first.clientid);
        assert_eq!(confirm(&sender, update, Credential::None).await, Ok(()));
        tokio::time::advance(Duration::from_secs(5)).await;
        assert_eq!(renew(&sender, first.clientid, foreign()).await, Err(Error::IdentityInUse));
        assert!(expired.try_recv().is_err());
        let reboot = register(&sender, 2, 1).await;
        assert_ne!(reboot.clientid, first.clientid);
        assert_eq!(confirm(&sender, reboot, foreign()).await, Err(Error::IdentityInUse));
        assert_eq!(confirm(&sender, reboot, Credential::None).await, Ok(()));
        tokio::time::advance(Duration::from_secs(6)).await;
        tokio::task::yield_now().await;
        assert!(expired.try_recv().is_err());
        tokio::time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;
        assert_eq!(expired.try_recv().unwrap(), reboot.clientid);
        assert!(expired.try_recv().is_err());
        drop(sender);
        running.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn invalidation_cancels_timer_without_affecting_reused_queue_keys() {
        let (task, sender, expired) = ClientIdTask::new(LEASE);
        let running = task.spawn();
        let old = unchecked(&sender, connection(), Credential::None).await.unwrap();
        assert_eq!(invalidate(&sender, old).await, Some(old));
        let next = unchecked(&sender, connection(), Credential::None).await.unwrap();
        assert_ne!(old, next);
        assert_eq!(invalidate(&sender, old).await, None);
        tokio::time::advance(LEASE + Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(expired.try_recv().unwrap(), next);
        assert!(expired.try_recv().is_err());
        drop(sender);
        running.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn invalidation_returns_removed_id_and_preserves_other_protocol() {
        let (task, sender, expired) = ClientIdTask::new(LEASE);
        let running = task.spawn();
        let peer = unchecked(&sender, connection(), Credential::None).await.unwrap();
        let first = register(&sender, 1, 1).await;
        assert_eq!(confirm(&sender, first, Credential::None).await, Ok(()));

        assert_eq!(invalidate(&sender, first.clientid).await, Some(first.clientid));
        assert_eq!(
            renew(&sender, first.clientid, Credential::None).await,
            Err(Error::StaleClientId)
        );
        assert_eq!(renew(&sender, peer, Credential::None).await, Ok(()));
        assert_eq!(invalidate(&sender, first.clientid).await, None);

        let next = register(&sender, 2, 1).await;
        assert_eq!(confirm(&sender, next, Credential::None).await, Ok(()));
        assert_eq!(invalidate(&sender, peer).await, Some(peer));
        assert_eq!(renew(&sender, next.clientid, Credential::None).await, Ok(()));
        tokio::time::advance(LEASE + Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(expired.try_recv().unwrap(), next.clientid);
        assert!(expired.try_recv().is_err());
        drop(sender);
        running.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn invalidating_unconfirmed_client_removes_pending_registration_without_timer() {
        let (task, sender, expired) = ClientIdTask::new(LEASE);
        let running = task.spawn();
        let pending = register(&sender, 1, 1).await;
        assert_eq!(invalidate(&sender, pending.clientid).await, None);
        assert_eq!(confirm(&sender, pending, Credential::None).await, Err(Error::StaleClientId));
        assert_eq!(invalidate(&sender, ClientId::new(u64::MAX)).await, None);
        tokio::time::advance(LEASE + Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert!(expired.try_recv().is_err());
        drop(sender);
        running.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn repeated_registration_cannot_change_lease_owner_or_extend_foreign_lease() {
        let (task, sender, expired) = ClientIdTask::new(LEASE);
        let running = task.spawn();
        let owner = foreign();
        let peer = unchecked(&sender, connection(), owner.clone()).await.unwrap();
        tokio::time::advance(Duration::from_secs(6)).await;
        assert_eq!(unchecked(&sender, connection(), owner.clone()).await, Ok(peer));
        let mut changed = owner.clone();
        if let Credential::Sys(params) = &mut changed {
            params.stamp += 1;
        }
        assert_ne!(changed, owner);
        tokio::time::advance(Duration::from_secs(5)).await;
        assert_eq!(
            unchecked(&sender, connection(), changed.clone()).await,
            Err(Error::IdentityInUse)
        );
        assert_eq!(renew(&sender, peer, changed).await, Err(Error::IdentityInUse));
        assert!(expired.try_recv().is_err());
        tokio::time::advance(Duration::from_secs(6)).await;
        tokio::task::yield_now().await;
        assert_eq!(expired.try_recv().unwrap(), peer);
        assert!(expired.try_recv().is_err());
        drop(sender);
        running.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn invalidating_already_removed_registration_cancels_remaining_timer() {
        let (mut task, _sender, _expired) = ClientIdTask::new(LEASE);
        let peer = task.clients.register_unchecked(connection(), &Credential::None).unwrap();
        task.leases.register(peer, Credential::None).unwrap();
        assert!(task.clients.invalidate(peer).is_some());
        assert_eq!(task.invalidate(peer), None);
        assert!(task.leases.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn zero_duration_expires_immediately() {
        let (task, sender, expired) = ClientIdTask::new(Duration::ZERO);
        let running = task.spawn();
        let peer = unchecked(&sender, connection(), Credential::None).await.unwrap();
        tokio::task::yield_now().await;
        assert_eq!(expired.try_recv().unwrap(), peer);
        assert_eq!(renew(&sender, peer, Credential::None).await, Err(Error::StaleClientId));
        drop(sender);
        running.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn missing_registration_still_notifies_cleanup_on_expiration() {
        let (mut task, sender, expired) = ClientIdTask::new(LEASE);
        let peer = task.clients.register_unchecked(connection(), &Credential::None).unwrap();
        task.leases.register(peer, Credential::None).unwrap();
        assert!(task.clients.invalidate(peer).is_some());
        let running = task.spawn();
        tokio::time::advance(LEASE + Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(expired.try_recv().unwrap(), peer);
        assert_eq!(renew(&sender, peer, Credential::None).await, Err(Error::StaleClientId));
        drop(sender);
        running.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn closed_expiration_channel_does_not_stop_actor_and_commands_drain_on_shutdown() {
        let (task, sender, expired) = ClientIdTask::new(LEASE);
        let running = task.spawn();
        drop(expired);
        let old = unchecked(&sender, connection(), Credential::None).await.unwrap();
        tokio::time::advance(LEASE + Duration::from_millis(1)).await;
        assert_eq!(renew(&sender, old, Credential::None).await, Err(Error::StaleClientId));
        let (result_sender, receiver) = oneshot::channel();
        sender
            .send(ClientIdCommand::RegisterUnchecked {
                connection: connection(),
                credentials: Credential::None,
                result_sender,
            })
            .await
            .unwrap();
        drop(sender);
        running.await.unwrap();
        assert_ne!(receiver.await.unwrap().unwrap(), old);
    }
}
