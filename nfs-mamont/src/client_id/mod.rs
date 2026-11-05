//! Client registration and connection identity.
//!
//! The registry implements RFC 3530 confirmed/unconfirmed transitions. Connection
//! metadata is represented by [`ConnectionInfo`] and [`ConnectionGeneration`].
//! [`ConnectionGenerationGenerator`] and the registry reuse the same private
//! incrementing counter implementation, with independent sequences. The accept
//! loop owns the connection generator; there is no global counter or static state.
//!
//! All mutations require exclusive access. Share a manager under the same lock as
//! the state it identifies. No NLM or NFS wire types are used here.
//! RPC authentication and authorization belong to the caller. Lease lifetimes,
//! expiration, and state cleanup belong to a separate lease/state manager.
//!
//! # Duplicate calls
//!
//! A bounded FIFO cache identifies calls by operation and exact arguments:
//! [`Registration`] for `register`, [`PendingClient`] for `confirm`. It accepts no
//! RPC XID or request identifier. Identical new calls and retries share a cached
//! result until eviction. A miss executes the current registration transition.
//! Cached confirmation replies never repeat retirement. Historical replies may
//! outlive invalidation or replacement; replaying them never restores a record.
//! Registration payloads are shared internally between the cache and tables,
//! without cloning identity or callback buffers. No lifetime parameters are
//! added to the public API.
//!
//! ```
//! use nfs_mamont::client_id::*;
//!
//! let mut clients = ClientIdManager::new();
//! let registration = Registration {
//!     identity: ClientIdentity::Handshake(b"stable-client".to_vec()),
//!     verifier: ClientVerifier::new([1; 8]), // Client incarnation verifier.
//!     callback: Callback::Universal {
//!         netid: NetId::new("tcp".into()), address: "192.0.2.1:2049".parse().unwrap(), program: 123456, ident: 1,
//!     },
//! };
//! let identity = registration.identity.clone();
//! let pair = clients.register(registration).unwrap();
//! let transition = clients.confirm(pair).unwrap();
//! assert!(transition.retired.is_none()); // Initial registration has nothing to retire.
//! assert_eq!(clients.lookup(&identity), Some(transition.clientid));
//! assert_eq!(clients.get(transition.clientid).unwrap().identity, identity);
//! ```

mod connection;
mod generator;
#[cfg(test)]
mod tests;
mod types;

pub use connection::{ConnectionGeneration, ConnectionGenerationGenerator, ConnectionInfo};
pub use types::{
    Callback, ClientId, ClientIdentity, ClientVerifier, ConfirmationVerifier, Error, HostName,
    NetId, PendingClient, Registration, Result, Transition,
};

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use generator::Generator;
use types::OPAQUE_LIMIT;

const DRC_CAPACITY: usize = 1024;

#[derive(Debug, PartialEq, Eq)]
struct ClientRecord {
    registration: Arc<Registration>,
    clientid: ClientId,
    confirmation: ConfirmationVerifier,
}

enum Cached {
    Register { registration: Arc<Registration>, result: Result<PendingClient> },
    Confirm { pair: PendingClient, result: Result<()> },
}

/// Standalone client identity registry. Authentication, lease management and state
/// cleanup are performed by the caller. Records remain until explicitly removed
/// or replaced. Construct using [`Self::new`]; [`Self::register`] registers a client.
#[derive(Default)]
pub struct ClientIdManager {
    ids: Generator,
    confirmations: Generator,
    confirmed: HashMap<ClientId, ClientRecord>,
    pending: HashMap<ClientId, ClientRecord>,
    drc: VecDeque<Cached>,
}

impl ClientIdManager {
    /// Creates an empty registry without configuration or a clock.
    pub fn new() -> Self {
        Self {
            ids: Generator::new(),
            confirmations: Generator::new(),
            confirmed: HashMap::new(),
            pending: HashMap::new(),
            drc: VecDeque::new(),
        }
    }

    /// SETCLIENTID transition. Exact argument repeats return the cached pair.
    /// A cache miss replaces the pending record, preserving confirmed state.
    pub fn register(&mut self, registration: Registration) -> Result<PendingClient> {
        let replay = self.drc.iter().find_map(|entry| match entry {
            Cached::Register { registration: cached, result }
                if cached.as_ref() == &registration =>
            {
                Some(*result)
            }
            _ => None,
        });
        if let Some(result) = replay {
            return result;
        }
        let registration = Arc::new(registration);
        let result = self.register_inner(&registration);
        self.cache(Cached::Register { registration, result });
        result
    }

    fn register_inner(&mut self, registration: &Arc<Registration>) -> Result<PendingClient> {
        let ClientIdentity::Handshake(bytes) = &registration.identity else {
            return Err(Error::InvalidIdentity);
        };
        if bytes.len() > OPAQUE_LIMIT {
            return Err(Error::InvalidIdentity);
        }
        let reuse = self
            .lookup_record(&registration.identity)
            .filter(|old| old.registration.verifier == registration.verifier)
            .map(|old| old.clientid);
        let clientid = match reuse {
            Some(id) => id,
            None => ClientId::new(self.ids.next()),
        };
        let confirmation = ConfirmationVerifier::new(self.confirmations.next().to_be_bytes());
        let replaced = self
            .pending
            .iter()
            .find(|(_, record)| record.registration.identity == registration.identity)
            .map(|(id, _)| *id);
        if let Some(id) = replaced {
            self.pending.remove(&id);
        }
        let record =
            ClientRecord { registration: Arc::clone(registration), clientid, confirmation };
        self.pending.insert(clientid, record);
        Ok(PendingClient { clientid, confirmation })
    }

    /// SETCLIENTID_CONFIRM transition. Repeats return the cached status without
    /// reporting retirement again. On a miss the current pair is checked.
    pub fn confirm(&mut self, pair: PendingClient) -> Result<Transition> {
        let replay = self.drc.iter().find_map(|entry| match entry {
            Cached::Confirm { pair: cached, result } if *cached == pair => Some(*result),
            _ => None,
        });
        if let Some(result) = replay {
            return result.map(|()| Transition { clientid: pair.clientid, retired: None });
        }
        let result = self.confirm_inner(pair);
        self.cache(Cached::Confirm { pair, result: result.map(|_| ()) });
        result
    }

    fn confirm_inner(&mut self, pair: PendingClient) -> Result<Transition> {
        let Some(pending) = self.pending.get(&pair.clientid) else {
            // RFC §14.2.34 allows a confirmed retry when no pending record shares c.
            let Some(record) = self.confirmed.get(&pair.clientid) else {
                return Err(Error::StaleClientId);
            };
            if record.confirmation != pair.confirmation {
                return Err(Error::StaleClientId);
            }
            return Ok(Transition { clientid: pair.clientid, retired: None });
        };
        if pending.confirmation != pair.confirmation {
            return Err(Error::StaleClientId);
        }
        let Some(record) = self.pending.remove(&pair.clientid) else {
            return Err(Error::StaleClientId);
        };
        let retired = self
            .lookup_record(&record.registration.identity)
            .map(|old| old.clientid)
            .filter(|old_id| *old_id != record.clientid);
        if let Some(old_id) = retired {
            self.confirmed.remove(&old_id);
        }
        self.confirmed.insert(record.clientid, record);
        Ok(Transition { clientid: pair.clientid, retired })
    }

    /// Get-or-insert for adapters without a handshake. A new incarnation retires
    /// the previous one immediately; repeated calls preserve the ID and callback.
    pub fn new_unchecked(&mut self, registration: Registration) -> Result<Transition> {
        if !matches!(&registration.identity, ClientIdentity::Implicit { .. }) {
            return Err(Error::InvalidIdentity);
        }
        if let Some(old) = self.lookup_record(&registration.identity) {
            if old.registration.verifier == registration.verifier {
                return Ok(Transition { clientid: old.clientid, retired: None });
            }
        }
        let clientid = ClientId::new(self.ids.next());
        let confirmation = ConfirmationVerifier::new(self.confirmations.next().to_be_bytes());
        let record = ClientRecord { registration: Arc::new(registration), clientid, confirmation };
        let retired = self.lookup_record(&record.registration.identity).map(|old| old.clientid);
        if let Some(old_id) = retired {
            self.confirmed.remove(&old_id);
        }
        self.confirmed.insert(clientid, record);
        Ok(Transition { clientid, retired })
    }

    /// Returns only confirmed registrations; unknown IDs are stale.
    pub fn get(&self, clientid: ClientId) -> Result<&Registration> {
        let Some(record) = self.confirmed.get(&clientid) else {
            return Err(Error::StaleClientId);
        };
        Ok(&record.registration)
    }

    /// Returns the confirmed shorthand for a logical identity in its namespace.
    pub fn lookup(&self, identity: &ClientIdentity) -> Option<ClientId> {
        self.lookup_record(identity).map(|record| record.clientid)
    }

    fn lookup_record(&self, identity: &ClientIdentity) -> Option<&ClientRecord> {
        self.confirmed.values().find(|record| &record.registration.identity == identity)
    }

    /// Removes records for exactly this shorthand, including a pending callback
    /// update sharing the ID. Pending registrations with a different ID survive.
    /// Returns the retired confirmed ID for the state layer, or `None` when only
    /// a pending record was removed or the ID was unknown.
    #[must_use = "the state layer must handle a retired confirmed client"]
    pub fn invalidate(&mut self, clientid: ClientId) -> Option<ClientId> {
        self.pending.remove(&clientid);
        self.confirmed.remove(&clientid).map(|record| record.clientid)
    }
    fn cache(&mut self, entry: Cached) {
        if self.drc.len() >= DRC_CAPACITY {
            self.drc.pop_front();
        }
        self.drc.push_back(entry);
    }
}
