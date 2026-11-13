//! Lease timers and their client records are mutated together through this module.

use std::collections::{hash_map::Entry, HashMap};
use std::future::poll_fn;
use std::task::{ready, Poll};
use std::time::Duration;

use tokio::time::Instant;
use tokio_util::time::{delay_queue::Key, DelayQueue};

use crate::auth::Credential;
use crate::client_id::{ClientId, Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Nfs3,
    Nfs4,
}

/// Metadata returned only after both the timer and its client record are removed.
pub struct RemovedLease {
    pub clientid: ClientId,
    pub protocol: Protocol,
    deadline: Instant,
}

impl RemovedLease {
    pub fn is_expired(&self) -> bool {
        self.deadline <= Instant::now()
    }
}

struct Lease {
    key: Key,
    protocol: Protocol,
    credentials: Credential,
}

/// Every active record owns exactly one timer. Keys and both collections remain
/// private so callers cannot remove a timer independently of its record.
pub struct ClientLeases {
    duration: Duration,
    leases: DelayQueue<ClientId>,
    active_leases: HashMap<ClientId, Lease>,
}

impl ClientLeases {
    pub fn new(duration: Duration) -> Self {
        Self { duration, leases: DelayQueue::new(), active_leases: HashMap::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.active_leases.is_empty()
    }

    /// Creates a timer or renews an existing lease without changing its owner.
    /// An overdue lease must be removed before the client can register again.
    pub fn register(
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
                let key = self.leases.insert(clientid, self.duration);
                entry.insert(Lease { key, protocol, credentials });
                Ok(())
            }
        }
    }

    /// Resets only the timer, after checking the deadline and credentials.
    pub fn renew(&mut self, clientid: ClientId, credentials: &Credential) -> Result<()> {
        let lease = self.active_leases.get(&clientid).ok_or(Error::StaleClientId)?;
        if self.leases.deadline(&lease.key) <= Instant::now() {
            return Err(Error::StaleClientId);
        }
        if lease.credentials != *credentials {
            return Err(Error::IdentityInUse);
        }
        self.leases.reset(&lease.key, self.duration);
        Ok(())
    }

    /// Removes the record and its timer together; an unknown ID has no effect.
    #[must_use]
    pub fn remove(&mut self, clientid: ClientId) -> Option<RemovedLease> {
        let entry = match self.active_leases.entry(clientid) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => return None,
        };
        let removed = self.leases.remove(&entry.get().key);
        let lease = entry.remove();
        Some(RemovedLease { clientid, protocol: lease.protocol, deadline: removed.deadline() })
    }

    /// Checks the deadline and removes an overdue lease in a single operation.
    #[must_use]
    pub fn remove_expired(&mut self, clientid: ClientId) -> Option<RemovedLease> {
        let entry = match self.active_leases.entry(clientid) {
            Entry::Occupied(entry) if self.leases.deadline(&entry.get().key) <= Instant::now() => {
                entry
            }
            _ => return None,
        };
        let removed = self.leases.remove(&entry.get().key);
        let lease = entry.remove();
        Some(RemovedLease { clientid, protocol: lease.protocol, deadline: removed.deadline() })
    }

    /// Waits for and removes the next expired lease. Returns `None` when empty.
    /// Cancellation leaves both collections unchanged unless a lease is returned.
    pub async fn expired(&mut self) -> Option<RemovedLease> {
        poll_fn(|cx| {
            let Some(expired) = ready!(self.leases.poll_expired(cx)) else {
                return Poll::Ready(None);
            };
            let deadline = expired.deadline();
            let clientid = expired.into_inner();
            let Some(lease) = self.active_leases.remove(&clientid) else {
                unreachable!("each lease timer is created and removed with its active record");
            };
            Poll::Ready(Some(RemovedLease { clientid, protocol: lease.protocol, deadline }))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthSysParams;
    use tokio::time::advance;

    const DURATION: Duration = Duration::from_secs(10);

    fn foreign() -> Credential {
        Credential::Sys(AuthSysParams {
            stamp: 1,
            machine_name: "host".into(),
            uid: 100,
            gid: 10,
            gids: vec![],
        })
    }

    fn assert_count(leases: &ClientLeases, count: usize) {
        assert_eq!(leases.active_leases.len(), count);
        assert_eq!(leases.leases.len(), count);
        assert_eq!(leases.is_empty(), count == 0);
    }

    #[tokio::test(start_paused = true)]
    async fn register_renew_and_remove_keep_records_and_timers_together() {
        let mut leases = ClientLeases::new(DURATION);
        let a = ClientId::new(1);
        let b = ClientId::new(2);
        leases.register(a, Protocol::Nfs3, Credential::None).unwrap();
        leases.register(b, Protocol::Nfs4, foreign()).unwrap();
        assert_count(&leases, 2);
        advance(Duration::from_secs(6)).await;
        leases.register(a, Protocol::Nfs3, Credential::None).unwrap();
        assert_count(&leases, 2);
        let removed = leases.remove(b).unwrap();
        assert_eq!(removed.clientid, b);
        assert_eq!(removed.protocol, Protocol::Nfs4);
        assert!(!removed.is_expired());
        assert_count(&leases, 1);
        assert!(leases.remove(b).is_none());
        assert_count(&leases, 1);
        advance(Duration::from_secs(5)).await;
        assert!(leases.remove_expired(a).is_none());
        assert_count(&leases, 1);
        advance(Duration::from_secs(6)).await;
        let removed = leases.remove_expired(a).unwrap();
        assert_eq!(removed.clientid, a);
        assert_eq!(removed.protocol, Protocol::Nfs3);
        assert!(removed.is_expired());
        assert_count(&leases, 0);
        assert!(leases.expired().await.is_none());

        // Reusing the queue's storage cannot leave an old key in the map.
        leases.register(b, Protocol::Nfs4, foreign()).unwrap();
        assert!(leases.remove(a).is_none());
        assert_count(&leases, 1);
        let expired = leases.expired().await.unwrap();
        assert_eq!(expired.clientid, b);
        assert_eq!(expired.protocol, Protocol::Nfs4);
        assert!(expired.is_expired());
        assert_count(&leases, 0);
        assert_eq!(leases.renew(b, &foreign()), Err(Error::StaleClientId));
    }

    #[tokio::test(start_paused = true)]
    async fn rejected_registration_preserves_protocol_credentials_and_deadline() {
        let mut leases = ClientLeases::new(DURATION);
        let id = ClientId::new(1);
        leases.register(id, Protocol::Nfs3, Credential::None).unwrap();
        let original_key = leases.active_leases[&id].key;
        let original_deadline = leases.leases.deadline(&original_key);
        advance(Duration::from_secs(3)).await;
        assert_eq!(
            leases.register(id, Protocol::Nfs4, Credential::None),
            Err(Error::StaleClientId)
        );
        assert_eq!(leases.register(id, Protocol::Nfs3, foreign()), Err(Error::IdentityInUse));
        assert_eq!(leases.renew(id, &foreign()), Err(Error::IdentityInUse));
        let lease = &leases.active_leases[&id];
        assert_eq!(lease.key, original_key);
        assert_eq!(lease.protocol, Protocol::Nfs3);
        assert_eq!(lease.credentials, Credential::None);
        assert_eq!(leases.leases.deadline(&lease.key), original_deadline);
        assert_count(&leases, 1);
        advance(DURATION).await;
        assert_eq!(leases.renew(id, &Credential::None), Err(Error::StaleClientId));
        assert_eq!(
            leases.register(id, Protocol::Nfs3, Credential::None),
            Err(Error::StaleClientId)
        );
        assert_count(&leases, 1);
        assert!(leases.remove_expired(id).unwrap().is_expired());
        assert_count(&leases, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_wait_and_renewal_leave_one_current_timer() {
        let mut leases = ClientLeases::new(DURATION);
        let id = ClientId::new(1);
        leases.register(id, Protocol::Nfs4, Credential::None).unwrap();
        tokio::select! {
            biased;
            _ = tokio::task::yield_now() => {}
            _ = leases.expired() => panic!("lease must not expire before its deadline"),
        }
        assert_count(&leases, 1);
        advance(Duration::from_secs(6)).await;
        leases.renew(id, &Credential::None).unwrap();
        advance(Duration::from_secs(5)).await;
        assert!(leases.remove_expired(id).is_none());
        assert_count(&leases, 1);
        let expired = leases.expired().await.unwrap();
        assert_eq!(expired.clientid, id);
        assert!(expired.is_expired());
        assert_count(&leases, 0);
        assert!(leases.expired().await.is_none());
    }
}
