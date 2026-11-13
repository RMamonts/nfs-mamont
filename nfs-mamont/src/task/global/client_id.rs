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

use std::{
    collections::{hash_map::Entry, HashMap},
    future::poll_fn,
    time::Duration,
};

use async_channel::{Receiver, Sender};
use tokio::time::Instant;
use tokio::{sync::oneshot, task::JoinHandle};
use tokio_util::time::{delay_queue::Key, DelayQueue};

use crate::{
    auth::Credential,
    client_id::{
        ClientId, ClientIdGenerator, ClientIdManager, ConnectionInfo, Error, Nfs3ClientIdManager,
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
    /// Extends an active lease after authenticated client activity.
    Renew { clientid: ClientId, credentials: Credential, result_sender: ClientIdReplySender<()> },
    /// Removes a client and cancels its timer. Returns the removed active ID for
    /// state cleanup, or `None` for an unconfirmed or unknown client.
    Invalidate { clientid: ClientId, result_sender: ClientIdReplySender<Option<ClientId>> },
}

/// Owns both client registries and their common ID allocator.
pub struct ClientIdTask {
    nfsv4: ClientIdManager,
    nfsv3: Nfs3ClientIdManager,
    receiver: ClientIdReceiver,
    lease_duration: Duration,
    leases: DelayQueue<ClientId>,
    active_leases: HashMap<ClientId, Lease>,
    expired_sender: Sender<ClientId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Protocol {
    Nfs3,
    Nfs4,
}

/// Timer handle, owning registry, and immutable principal allowed to renew it.
struct Lease {
    key: Key,
    protocol: Protocol,
    credentials: Credential,
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
        let ids = ClientIdGenerator::new();
        let task = Self {
            nfsv4: ClientIdManager::new(ids.clone()),
            nfsv3: Nfs3ClientIdManager::new(ids),
            receiver,
            lease_duration,
            leases: DelayQueue::new(),
            active_leases: HashMap::new(),
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
                Some(expired) = poll_fn(|cx| self.leases.poll_expired(cx)), if !self.leases.is_empty() => {
                    let key = expired.key();
                    let clientid = expired.into_inner();
                    match self.active_leases.entry(clientid) {
                        Entry::Occupied(entry) if entry.get().key == key => {
                            let lease = entry.remove();
                            self.expire(clientid, lease);
                        }
                        Entry::Occupied(_) => {
                            // An obsolete timer must not expire the current lease.
                            tracing::debug!(clientid = clientid.into_inner(), "ignoring obsolete lease timer");
                        }
                        Entry::Vacant(_) => unreachable!("expired timer must have an active lease"),
                    }
                }
            }
        }
    }

    fn handle(&mut self, command: ClientIdCommand) {
        match command {
            ClientIdCommand::Register { registration, credentials, result_sender } => {
                if let Some(clientid) = self.nfsv4.lookup(&registration.identity) {
                    self.expire_if_due(clientid);
                }
                let result = self.nfsv4.register(registration, &credentials);
                send_reply(result_sender, result);
            }
            ClientIdCommand::Confirm { pair, credentials, result_sender } => {
                self.expire_if_due(pair.clientid);
                let result = self.nfsv4.confirm(pair, &credentials).and_then(|retired| {
                    // Confirmation DRC may replay success after invalidation.
                    // Preserve the reply, but never revive an expired client.
                    if let Some(old) = retired {
                        // The manager already removed the old registration.
                        let overdue = self.lease_is_due(old);
                        match self.cancel_lease(old) {
                            Some(_) if overdue => self.notify_expired(old),
                            Some(_) => {}
                            None => unreachable!("retired confirmed client must have a lease"),
                        }
                    }
                    if self.nfsv4.get(pair.clientid).is_ok() {
                        self.register_lease(pair.clientid, Protocol::Nfs4, credentials)?;
                    }
                    Ok(())
                });
                send_reply(result_sender, result);
            }
            ClientIdCommand::RegisterUnchecked { connection, credentials, result_sender } => {
                if let Some(clientid) = self.nfsv3.lookup(connection) {
                    self.expire_if_due(clientid);
                }
                let result = self.nfsv3.register(connection, &credentials).and_then(|clientid| {
                    self.register_lease(clientid, Protocol::Nfs3, credentials)?;
                    Ok(clientid)
                });
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

    /// Credentials and protocol are set only when the lease is first created.
    fn register_lease(
        &mut self,
        clientid: ClientId,
        protocol: Protocol,
        credentials: Credential,
    ) -> Result<()> {
        match self.active_leases.entry(clientid) {
            Entry::Occupied(entry) => {
                if entry.get().protocol != protocol {
                    return Err(Error::StaleClientId);
                }
                self.renew(clientid, &credentials)
            }
            Entry::Vacant(entry) => {
                let key = self.leases.insert(clientid, self.lease_duration);
                entry.insert(Lease { key, protocol, credentials });
                Ok(())
            }
        }
    }

    #[must_use]
    fn cancel_lease(&mut self, clientid: ClientId) -> Option<Lease> {
        let lease = self.active_leases.remove(&clientid)?;
        // The key is owned by this lease and is consumed along with it.
        self.leases.remove(&lease.key);
        Some(lease)
    }

    #[must_use]
    fn invalidate(&mut self, clientid: ClientId) -> Option<ClientId> {
        if let Some(lease) = self.cancel_lease(clientid) {
            self.remove_registration(clientid, lease.protocol)
        } else {
            // Only NFSv4 has unconfirmed registrations without an active lease.
            self.nfsv4.invalidate(clientid)
        }
    }

    fn remove_registration(&mut self, clientid: ClientId, protocol: Protocol) -> Option<ClientId> {
        match protocol {
            Protocol::Nfs3 => self.nfsv3.invalidate(clientid).map(|_| clientid),
            Protocol::Nfs4 => self.nfsv4.invalidate(clientid),
        }
    }

    fn expire(&mut self, clientid: ClientId, lease: Lease) {
        if self.remove_registration(clientid, lease.protocol).is_none() {
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

    fn lease_is_due(&self, clientid: ClientId) -> bool {
        self.active_leases
            .get(&clientid)
            .is_some_and(|lease| self.leases.deadline(&lease.key) <= Instant::now())
    }

    fn expire_if_due(&mut self, clientid: ClientId) {
        if self.lease_is_due(clientid) {
            let Some(lease) = self.cancel_lease(clientid) else {
                unreachable!("overdue client must have an active lease");
            };
            self.expire(clientid, lease);
        }
    }

    fn renew(&mut self, clientid: ClientId, credentials: &Credential) -> Result<()> {
        self.expire_if_due(clientid);
        let lease = self.active_leases.get(&clientid).ok_or(Error::StaleClientId)?;
        if lease.credentials != *credentials {
            return Err(Error::IdentityInUse);
        }
        self.leases.reset(&lease.key, self.lease_duration);
        Ok(())
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
        let first = task.nfsv4.register(registration(1, 1), &Credential::None).unwrap();
        task.nfsv4.confirm(first, &Credential::None).unwrap();
        task.register_lease(first.clientid, Protocol::Nfs4, Credential::None).unwrap();
        let reboot = task.nfsv4.register(registration(2, 1), &Credential::None).unwrap();
        tokio::time::advance(LEASE + Duration::from_millis(1)).await;
        let (result_sender, receiver) = oneshot::channel();
        task.handle(ClientIdCommand::Confirm {
            pair: reboot,
            credentials: Credential::None,
            result_sender,
        });
        assert_eq!(receiver.await.unwrap(), Ok(()));
        assert_eq!(expired.try_recv().unwrap(), first.clientid);
        assert_eq!(task.active_leases.len(), 1);
        assert!(task.active_leases.contains_key(&reboot.clientid));
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
        let peer = task.nfsv3.register(connection(), &Credential::None).unwrap();
        task.register_lease(peer, Protocol::Nfs3, Credential::None).unwrap();
        assert!(task.nfsv3.invalidate(peer).is_some());
        assert_eq!(task.invalidate(peer), None);
        assert!(task.active_leases.is_empty());
        assert!(task.leases.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn protocol_mismatch_is_rejected_without_changing_lease() {
        let (mut task, _sender, _expired) = ClientIdTask::new(LEASE);
        let peer = task.nfsv3.register(connection(), &Credential::None).unwrap();
        task.register_lease(peer, Protocol::Nfs3, Credential::None).unwrap();
        let deadline = task.leases.deadline(&task.active_leases[&peer].key);
        tokio::time::advance(Duration::from_secs(3)).await;
        assert_eq!(task.register_lease(peer, Protocol::Nfs4, foreign()), Err(Error::StaleClientId));
        let lease = &task.active_leases[&peer];
        assert_eq!(lease.protocol, Protocol::Nfs3);
        assert_eq!(lease.credentials, Credential::None);
        assert_eq!(task.leases.deadline(&lease.key), deadline);
        assert_eq!(task.invalidate(peer), Some(peer));
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
    async fn obsolete_timer_does_not_expire_current_lease() {
        let (mut task, sender, expired) = ClientIdTask::new(LEASE);
        let peer = task.nfsv3.register(connection(), &Credential::None).unwrap();
        task.register_lease(peer, Protocol::Nfs3, Credential::None).unwrap();
        task.leases.insert(peer, Duration::from_secs(1));
        let running = task.spawn();
        tokio::time::advance(Duration::from_secs(2)).await;
        tokio::task::yield_now().await;
        assert!(expired.try_recv().is_err());
        assert_eq!(renew(&sender, peer, Credential::None).await, Ok(()));
        tokio::time::advance(LEASE + Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(expired.try_recv().unwrap(), peer);
        assert!(expired.try_recv().is_err());
        drop(sender);
        running.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn missing_registration_still_notifies_cleanup_on_expiration() {
        let (mut task, sender, expired) = ClientIdTask::new(LEASE);
        let peer = task.nfsv3.register(connection(), &Credential::None).unwrap();
        task.register_lease(peer, Protocol::Nfs3, Credential::None).unwrap();
        assert!(task.nfsv3.invalidate(peer).is_some());
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
