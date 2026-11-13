//! Lease timers and their client records are mutated together through this module.

use std::collections::{hash_map::Entry, HashMap};
use std::future::poll_fn;
use std::task::{ready, Poll};
use std::time::Duration;

use tokio::time::Instant;
use tokio_util::time::{delay_queue::Key, DelayQueue};

use crate::auth::Credential;
use crate::client_id::{ClientId, Error, Result};

/// Metadata returned only after both the timer and its client record are removed.
pub struct RemovedLease {
    pub clientid: ClientId,
    deadline: Instant,
}

impl RemovedLease {
    /// Whether the original deadline has elapsed at the time of this call.
    /// This also works for leases removed explicitly before their deadline.
    pub fn is_expired(&self) -> bool {
        self.deadline <= Instant::now()
    }
}

struct Lease {
    key: Key,
    credentials: Credential,
    /// Selected by the registration path and kept unchanged on renewal.
    timeout: Duration,
}

/// Every active record owns exactly one timer. Keys and both collections remain
/// private so callers cannot remove a timer independently of its record.
/// Checked and unchecked registration select the timeout for a new lease;
/// subsequent registration and renewal preserve its timeout and credentials.
/// This type does not manage registration records or release client resources.
pub struct ClientLeases {
    checked_timeout: Duration,
    unchecked_timeout: Duration,
    leases: DelayQueue<ClientId>,
    active_leases: HashMap<ClientId, Lease>,
}

impl ClientLeases {
    /// Configures independent timeouts for checked and unchecked registrations.
    /// A zero timeout makes new leases on that path expire immediately.
    pub fn new(checked_timeout: Duration, unchecked_timeout: Duration) -> Self {
        Self {
            checked_timeout,
            unchecked_timeout,
            leases: DelayQueue::new(),
            active_leases: HashMap::new(),
        }
    }

    /// Whether there are no active records or pending lease timers.
    pub fn is_empty(&self) -> bool {
        self.active_leases.is_empty()
    }

    /// Creates a checked lease or renews it with its original timeout and owner.
    /// An overdue lease must be removed before the client can register again.
    pub fn register_checked(&mut self, clientid: ClientId, credentials: Credential) -> Result<()> {
        self.register(clientid, credentials, self.checked_timeout)
    }

    /// Creates an unchecked lease or renews it with its original timeout and owner.
    /// An overdue lease must be removed before the client can register again.
    pub fn register_unchecked(
        &mut self,
        clientid: ClientId,
        credentials: Credential,
    ) -> Result<()> {
        self.register(clientid, credentials, self.unchecked_timeout)
    }

    fn register(
        &mut self,
        clientid: ClientId,
        credentials: Credential,
        timeout: Duration,
    ) -> Result<()> {
        match self.active_leases.entry(clientid) {
            Entry::Occupied(_) => self.renew(clientid, &credentials),
            Entry::Vacant(entry) => {
                let key = self.leases.insert(clientid, timeout);
                entry.insert(Lease { key, credentials, timeout });
                Ok(())
            }
        }
    }

    /// Resets the timer using the timeout selected at creation, after checking
    /// the deadline and credentials. No registration kind is needed to renew.
    pub fn renew(&mut self, clientid: ClientId, credentials: &Credential) -> Result<()> {
        let lease = self.active_leases.get(&clientid).ok_or(Error::StaleClientId)?;
        if self.leases.deadline(&lease.key) <= Instant::now() {
            return Err(Error::StaleClientId);
        }
        if lease.credentials != *credentials {
            return Err(Error::IdentityInUse);
        }
        self.leases.reset(&lease.key, lease.timeout);
        Ok(())
    }

    /// Removes the record and its timer together; an unknown ID has no effect.
    /// Returns metadata for caller-side cleanup without sending a notification.
    #[must_use]
    pub fn remove(&mut self, clientid: ClientId) -> Option<RemovedLease> {
        let entry = match self.active_leases.entry(clientid) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => return None,
        };
        let removed = self.leases.remove(&entry.get().key);
        entry.remove();
        Some(RemovedLease { clientid, deadline: removed.deadline() })
    }

    /// Checks the deadline and removes an overdue lease in a single operation.
    /// Unknown IDs and leases whose deadlines have not elapsed return `None`.
    /// The caller handles registration removal and expiration notification.
    #[must_use]
    pub fn remove_expired(&mut self, clientid: ClientId) -> Option<RemovedLease> {
        let entry = match self.active_leases.entry(clientid) {
            Entry::Occupied(entry) if self.leases.deadline(&entry.get().key) <= Instant::now() => {
                entry
            }
            _ => return None,
        };
        let removed = self.leases.remove(&entry.get().key);
        entry.remove();
        Some(RemovedLease { clientid, deadline: removed.deadline() })
    }

    /// Waits for and removes the next expired lease. Returns `None` when empty.
    /// The caller handles registration removal and expiration notification.
    /// Cancellation leaves both collections unchanged unless a lease is returned.
    pub async fn expired(&mut self) -> Option<RemovedLease> {
        poll_fn(|cx| {
            let Some(expired) = ready!(self.leases.poll_expired(cx)) else {
                return Poll::Ready(None);
            };
            let deadline = expired.deadline();
            let clientid = expired.into_inner();
            let Some(_) = self.active_leases.remove(&clientid) else {
                unreachable!("each lease timer is created and removed with its active record");
            };
            Poll::Ready(Some(RemovedLease { clientid, deadline }))
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
        let mut leases = ClientLeases::new(DURATION, DURATION);
        let a = ClientId::new(1);
        let b = ClientId::new(2);
        leases.register_checked(a, Credential::None).unwrap();
        leases.register_unchecked(b, foreign()).unwrap();
        assert_count(&leases, 2);
        advance(Duration::from_secs(6)).await;
        leases.register_checked(a, Credential::None).unwrap();
        assert_count(&leases, 2);
        let removed = leases.remove(b).unwrap();
        assert_eq!(removed.clientid, b);
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
        assert!(removed.is_expired());
        assert_count(&leases, 0);
        assert!(leases.expired().await.is_none());

        // Reusing the queue's storage cannot leave an old key in the map.
        leases.register_unchecked(b, foreign()).unwrap();
        assert!(leases.remove(a).is_none());
        assert_count(&leases, 1);
        let expired = leases.expired().await.unwrap();
        assert_eq!(expired.clientid, b);
        assert!(expired.is_expired());
        assert_count(&leases, 0);
        assert_eq!(leases.renew(b, &foreign()), Err(Error::StaleClientId));
    }

    #[tokio::test(start_paused = true)]
    async fn rejected_registration_preserves_credentials_and_deadline() {
        let mut leases = ClientLeases::new(DURATION, DURATION);
        let id = ClientId::new(1);
        leases.register_checked(id, Credential::None).unwrap();
        let original_key = leases.active_leases[&id].key;
        let original_deadline = leases.leases.deadline(&original_key);
        advance(Duration::from_secs(3)).await;
        assert_eq!(leases.register_checked(id, foreign()), Err(Error::IdentityInUse));
        assert_eq!(leases.renew(id, &foreign()), Err(Error::IdentityInUse));
        let lease = &leases.active_leases[&id];
        assert_eq!(lease.key, original_key);
        assert_eq!(lease.credentials, Credential::None);
        assert_eq!(leases.leases.deadline(&lease.key), original_deadline);
        assert_count(&leases, 1);
        advance(DURATION).await;
        assert_eq!(leases.renew(id, &Credential::None), Err(Error::StaleClientId));
        assert_eq!(leases.register_checked(id, Credential::None), Err(Error::StaleClientId));
        assert_count(&leases, 1);
        assert!(leases.remove_expired(id).unwrap().is_expired());
        assert_count(&leases, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn distinct_timeouts_are_preserved_when_both_kinds_are_renewed() {
        let mut leases = ClientLeases::new(Duration::from_secs(5), Duration::from_secs(12));
        let checked = ClientId::new(1);
        let unchecked = ClientId::new(2);
        leases.register_checked(checked, Credential::None).unwrap();
        leases.register_unchecked(unchecked, Credential::None).unwrap();
        advance(Duration::from_secs(3)).await;
        leases.renew(checked, &Credential::None).unwrap();
        leases.register_unchecked(unchecked, Credential::None).unwrap();
        assert_count(&leases, 2);

        advance(Duration::from_secs(6)).await;
        assert_eq!(leases.remove_expired(checked).unwrap().clientid, checked);
        assert!(leases.remove_expired(unchecked).is_none());
        assert_count(&leases, 1);
        leases.renew(unchecked, &Credential::None).unwrap();
        advance(Duration::from_secs(10)).await;
        assert!(leases.remove_expired(unchecked).is_none());
        advance(Duration::from_secs(3)).await;
        assert_eq!(leases.remove_expired(unchecked).unwrap().clientid, unchecked);
        assert_count(&leases, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn zero_timeout_affects_only_its_registration_kind() {
        for (checked_timeout, unchecked_timeout) in
            [(Duration::ZERO, DURATION), (DURATION, Duration::ZERO)]
        {
            let mut leases = ClientLeases::new(checked_timeout, unchecked_timeout);
            let checked = ClientId::new(1);
            let unchecked = ClientId::new(2);
            leases.register_checked(checked, Credential::None).unwrap();
            leases.register_unchecked(unchecked, Credential::None).unwrap();
            let (immediate, delayed) =
                if checked_timeout.is_zero() { (checked, unchecked) } else { (unchecked, checked) };
            assert_eq!(leases.remove_expired(immediate).unwrap().clientid, immediate);
            assert!(leases.remove_expired(delayed).is_none());
            assert_eq!(leases.renew(delayed, &Credential::None), Ok(()));
            assert_count(&leases, 1);
            advance(DURATION + Duration::from_millis(1)).await;
            assert_eq!(leases.remove_expired(delayed).unwrap().clientid, delayed);
            assert_count(&leases, 0);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_wait_and_renewal_leave_one_current_timer() {
        let mut leases = ClientLeases::new(DURATION, DURATION);
        let id = ClientId::new(1);
        leases.register_checked(id, Credential::None).unwrap();
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
