//! NFSv4 client registration (RFC 3530 §§14.2.33–14.2.34).
//!
//! Registration arguments contain {v, x, k}; records additionally hold the server
//! clientid c, confirmation verifier s and the RPC credentials that registered x.
//! Registration and confirmation first check caches of arguments and credentials.
//! Cached confirmation replies preserve the original status without changing
//! records, even after the confirmed record is removed.
//! Matching calls reuse the cached pair while that pending registration exists.
//! Confirmation, replacement and invalidation evict its cached registration.
//! Without RPC request metadata, identical new calls during that interval are
//! indistinguishable from retries. Complete RPC reply caching remains external.
//!
//! All mutations require exclusive access. Lease expiration and cleanup of locks,
//! shares and delegations belong to the external state manager and must be
//! coordinated with registration changes under the same lock.
//!
//! ```
//! use nfs_mamont::{auth::Credential, client_id::*};
//! let mut clients = ClientIdManager::new(ClientIdGenerator::new());
//! let registration = Registration {
//!     identity: ClientIdentity::new(b"stable-client".to_vec()).unwrap(),
//!     verifier: ClientVerifier::new([1; 8]),
//!     callback: Callback {
//!         netid: NetId::new("tcp".into()), address: "192.0.2.1:2049".parse().unwrap(),
//!         program: 123456, ident: 1,
//!     },
//! };
//! let pair = clients.register(registration, &Credential::None).unwrap();
//! clients.confirm(pair, &Credential::None).unwrap();
//! ```

#[cfg(test)]
mod tests;

use super::{
    cache::Cache,
    generator::{ClientIdGenerator, Generator},
    types::{
        ClientId, ClientIdentity, ConfirmationVerifier, Error, PendingClient, Registration, Result,
    },
};
use crate::auth::Credential;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
struct ClientRecord {
    registration: Registration,
    credentials: Credential,
    clientid: ClientId,
    confirmation: ConfirmationVerifier,
}

struct Cached {
    registration: Registration,
    credentials: Credential,
    pending: PendingClient,
}

struct CachedConfirmation {
    pair: PendingClient,
    credentials: Credential,
    result: Result<()>,
}

/// NFSv4 confirmed and unconfirmed client records, without lease management.
#[derive(Default)]
pub struct ClientIdManager {
    ids: ClientIdGenerator,
    confirmations: Generator,
    confirmed: HashMap<ClientId, ClientRecord>,
    pending: HashMap<ClientId, ClientRecord>,
    drc: Cache<Cached>,
    confirmation_drc: Cache<CachedConfirmation>,
}

impl ClientIdManager {
    /// Creates empty tables using the server's shared clientid allocator.
    pub fn new(ids: ClientIdGenerator) -> Self {
        Self { ids, ..Self::default() }
    }

    /// Processes SETCLIENTID, starting with the DRC lookup (§14.2.33).
    /// Checks the confirmed principal before any registration mutation. The
    /// confirmed registration and its leased state survive until confirmation.
    pub fn register(
        &mut self,
        registration: Registration,
        credentials: &Credential,
    ) -> Result<PendingClient> {
        // Step 1: return the cached response without changing any client state.
        if let Some(cached) = self.drc.find(|cached| {
            cached.registration == registration && cached.credentials == *credentials
        }) {
            return Ok(cached.pending);
        }
        let pending = self.register_inner(&registration, credentials)?;
        // A changed registration for x evicts previous responses for x, so an
        // earlier callback request cannot mask a subsequent callback update.
        self.drc.retain(|cached| cached.registration.identity != registration.identity);
        self.confirmation_drc.retain(|cached| cached.pair.clientid != pending.clientid);
        self.drc.add(Cached { registration, credentials: credentials.clone(), pending });
        Ok(pending)
    }

    fn register_inner(
        &mut self,
        registration: &Registration,
        credentials: &Credential,
    ) -> Result<PendingClient> {
        let clientid;
        if let Some(confirmed) = self.lookup_record(&registration.identity) {
            // Principal check precedes all four registration cases.
            if confirmed.credentials != *credentials {
                return Err(Error::IdentityInUse);
            }
            if confirmed.registration.verifier == registration.verifier {
                // Case 1: callback update {v, x, c, k, t}; preserve c.
                // Insertion below replaces only a pending record sharing c.
                clientid = confirmed.clientid;
            } else {
                // Cases 2 and 3: new incarnation, with or without pending state.
                // Replace pending records for x even if their verifier matches v.
                self.pending
                    .retain(|_, record| record.registration.identity != registration.identity);
                clientid = self.ids.allocate();
            }
        } else {
            // Case 4: no confirmed x; replace any unconfirmed x with fresh c,s.
            self.pending.retain(|_, record| record.registration.identity != registration.identity);
            clientid = self.ids.allocate();
        }
        let confirmation = ConfirmationVerifier::new(self.confirmations.next());
        let record = ClientRecord {
            registration: registration.clone(),
            credentials: credentials.clone(),
            clientid,
            confirmation,
        };
        self.pending.insert(clientid, record);
        Ok(PendingClient { clientid, confirmation })
    }

    /// Processes SETCLIENTID_CONFIRM, starting with the DRC lookup (§14.2.34).
    /// A principal mismatch preserves all records. Callback updates require
    /// distinct confirmed/unconfirmed verifiers before checking credentials.
    pub fn confirm(&mut self, pair: PendingClient, credentials: &Credential) -> Result<()> {
        if let Some(cached) = self
            .confirmation_drc
            .find(|cached| cached.pair == pair && cached.credentials == *credentials)
        {
            return cached.result;
        }
        let result = self.confirm_inner(pair, credentials);
        self.confirmation_drc.add(CachedConfirmation {
            pair,
            credentials: credentials.clone(),
            result,
        });
        result
    }

    fn confirm_inner(&mut self, pair: PendingClient, credentials: &Credential) -> Result<()> {
        let Some(pending) = self.pending.get(&pair.clientid) else {
            // Case 2: confirmed retry after the RPC cache entry was evicted.
            let Some(record) = self.confirmed.get(&pair.clientid) else {
                return Err(Error::StaleClientId);
            };
            if record.confirmation != pair.confirmation {
                return Err(Error::StaleClientId);
            }
            if record.credentials != *credentials {
                return Err(Error::IdentityInUse);
            }
            return Ok(());
        };
        if pending.confirmation != pair.confirmation {
            return Err(Error::StaleClientId);
        }
        // Case 1: {v,x,c,k,s} and {v,x,c,l,t} require s != t.
        // Check this condition before either record's principal.
        if let Some(confirmed) = self.confirmed.get(&pair.clientid) {
            if pending.confirmation == confirmed.confirmation {
                return Err(Error::StaleClientId);
            }
            if pending.credentials != *credentials || confirmed.credentials != *credentials {
                return Err(Error::IdentityInUse);
            }
        } else if pending.credentials != *credentials {
            // Case 3: only the unconfirmed record shares this c.
            return Err(Error::IdentityInUse);
        }
        let Some(record) = self.pending.remove(&pair.clientid) else {
            return Err(Error::StaleClientId);
        };
        // Remove the old confirmed record before promoting the pending record.
        let previous = self.lookup_record(&record.registration.identity).map(|old| old.clientid);
        if let Some(old) = previous {
            self.confirmed.remove(&old);
        }
        self.drc.retain(|cached| cached.pending.clientid != pair.clientid);
        self.confirmed.insert(pair.clientid, record);
        Ok(())
    }

    /// Returns a confirmed registration; unknown or unconfirmed IDs are stale.
    pub fn get(&self, clientid: ClientId) -> Result<&Registration> {
        let Some(record) = self.confirmed.get(&clientid) else {
            return Err(Error::StaleClientId);
        };
        Ok(&record.registration)
    }

    /// Finds the confirmed clientid for an opaque client.id.
    pub fn lookup(&self, identity: &ClientIdentity) -> Option<ClientId> {
        self.lookup_record(identity).map(|record| record.clientid)
    }

    fn lookup_record(&self, identity: &ClientIdentity) -> Option<&ClientRecord> {
        self.confirmed.values().find(|record| &record.registration.identity == identity)
    }

    /// Removes records sharing this clientid. The state layer must decide when
    /// releasing it is permitted and handle any returned confirmed clientid.
    #[must_use = "the state layer must handle a retired confirmed client"]
    pub fn invalidate(&mut self, clientid: ClientId) -> Option<ClientId> {
        self.drc.retain(|cached| cached.pending.clientid != clientid);
        self.pending.remove(&clientid);
        self.confirmed.remove(&clientid).map(|record| record.clientid)
    }
}
