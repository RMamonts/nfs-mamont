//! Checked and unchecked registries share one ID sequence and expose one removal API.
//!
//! Checked registration requires a separate confirmation step; unchecked
//! registration is immediately usable. Both paths validate credential ownership.
//! This module owns registration records and retry caches. The caller manages
//! lease timers and cleanup of locks and callbacks.

use crate::auth::Credential;
use crate::client_id::{
    ClientId, ClientIdGenerator, ClientIdManager as CheckedClientIdManager, ClientIdentity,
    ConnectionInfo, Nfs3ClientIdManager as UncheckedClientIdManager, PendingClient, Registration,
    Result,
};

/// Result of a successful checked confirmation, including cached replies.
pub struct Confirmation {
    /// Currently confirmed client to create or renew a lease for. `None` means
    /// success was replayed from the cache after the registration was removed.
    pub clientid: Option<ClientId>,
    /// Previous client ID removed by this confirmation. The caller must retire
    /// its lease and associated state. Callback updates and retries return `None`.
    pub retired: Option<ClientId>,
}

/// The registration path determines which registry owns a client. Managers and
/// their shared allocator remain private so callers cannot mix ownership.
///
/// A registered clientid belongs to exactly one manager: checked or unchecked.
/// The same clientid must never exist in both managers. Both managers use a
/// shared ID sequence, and each registration method writes to only one of them.
/// Checked records may be pending or confirmed; unchecked records need no
/// confirmation. Registry operations do not create, renew, or expire leases.
pub struct ClientRegistries {
    checked: CheckedClientIdManager,
    unchecked: UncheckedClientIdManager,
}

impl ClientRegistries {
    /// Creates empty checked and unchecked registries sharing one clientid allocator.
    pub fn new() -> Self {
        let ids = ClientIdGenerator::new();
        Self {
            checked: CheckedClientIdManager::new(ids.clone()),
            unchecked: UncheckedClientIdManager::new(ids),
        }
    }

    /// Finds a confirmed checked client by its logical identity. Pending-only
    /// registrations are not returned; lookup does not renew a lease.
    pub fn lookup(&self, identity: &ClientIdentity) -> Option<ClientId> {
        self.checked.lookup(identity)
    }

    /// Finds an unchecked client by the connection's peer address. Connection
    /// generation is ignored; lookup does not renew a lease.
    pub fn lookup_unchecked(&self, connection: ConnectionInfo) -> Option<ClientId> {
        self.unchecked.lookup(connection)
    }

    /// Returns a pending checked registration pair for [`Self::confirm`]. A retry
    /// may return the same pair. Any existing confirmed client remains usable
    /// until confirmation replaces it; this call does not start or renew a lease.
    pub fn register(
        &mut self,
        registration: Registration,
        credentials: &Credential,
    ) -> Result<PendingClient> {
        self.checked.register(registration, credentials)
    }

    /// Registers an unchecked peer immediately, without a confirmation step.
    /// The same peer address and credentials reuse the existing ID; different
    /// credentials are rejected. The caller must create or renew its lease.
    pub fn register_unchecked(
        &mut self,
        connection: ConnectionInfo,
        credentials: &Credential,
    ) -> Result<ClientId> {
        self.unchecked.register(connection, credentials)
    }

    /// Confirms a checked registration after validating the pair and credentials,
    /// or replays a cached reply. [`Confirmation::clientid`] identifies a current
    /// registration to create or renew a lease for; a cached success can have none.
    /// [`Confirmation::retired`] reports an ID only when this call removes it,
    /// so replaying a successful reply does not retire the previous client twice.
    pub fn confirm(
        &mut self,
        pair: PendingClient,
        credentials: &Credential,
    ) -> Result<Confirmation> {
        let retired = self.checked.confirm(pair, credentials)?;
        let clientid = self.checked.get(pair.clientid).ok().map(|_| pair.clientid);
        Ok(Confirmation { clientid, retired })
    }

    /// Removes a checked or unchecked client from its owning registry only.
    ///
    /// Returns `Some(clientid)` for a removed unchecked or confirmed checked
    /// client. Pending checked records sharing the ID are also removed, but
    /// removing only a pending record returns `None`, as does an unknown ID.
    /// The caller must cancel the lease and clean up associated state. Cached
    /// confirmation replies are retained and may still replay success.
    #[must_use]
    pub fn invalidate(&mut self, clientid: ClientId) -> Option<ClientId> {
        if self.unchecked.get(clientid).is_ok() {
            self.unchecked.invalidate(clientid).map(|_| clientid)
        } else {
            self.checked.invalidate(clientid)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthSysParams;
    use crate::client_id::{Callback, ClientVerifier, ConnectionGenerationGenerator, Error, NetId};

    fn registration(verifier: u8) -> Registration {
        Registration {
            identity: ClientIdentity::new(b"client".to_vec()).unwrap(),
            verifier: ClientVerifier::new([verifier; 8]),
            callback: Callback {
                netid: NetId::new("tcp".into()),
                address: "192.0.2.1:2049".parse().unwrap(),
                program: 123,
                ident: 1,
            },
        }
    }

    fn connection() -> ConnectionInfo {
        ConnectionInfo::new(
            "192.0.2.1:40000".parse().unwrap(),
            ConnectionGenerationGenerator::new().allocate(),
        )
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

    #[test]
    fn registration_paths_share_ids_and_removal_preserves_the_other_registry() {
        let mut clients = ClientRegistries::new();
        let registration = registration(1);
        let identity = registration.identity.clone();
        let checked = clients.register(registration, &Credential::None).unwrap();
        assert_eq!(clients.lookup(&identity), None);
        assert_eq!(
            clients.confirm(checked, &Credential::None).unwrap().clientid,
            Some(checked.clientid)
        );
        let unchecked = clients.register_unchecked(connection(), &Credential::None).unwrap();
        assert_eq!(unchecked.into_inner(), checked.clientid.into_inner() + 1);
        assert_eq!(clients.register_unchecked(connection(), &Credential::None), Ok(unchecked));
        assert_eq!(clients.invalidate(unchecked), Some(unchecked));
        assert_eq!(clients.lookup_unchecked(connection()), None);
        assert_eq!(clients.lookup(&identity), Some(checked.clientid));
        let next = clients.register_unchecked(connection(), &Credential::None).unwrap();
        assert_ne!(next, unchecked);
        assert_eq!(clients.invalidate(checked.clientid), Some(checked.clientid));
        assert_eq!(clients.lookup(&identity), None);
        assert_eq!(clients.lookup_unchecked(connection()), Some(next));
        assert_eq!(clients.invalidate(checked.clientid), None);
        assert_eq!(clients.invalidate(next), Some(next));
    }

    #[test]
    fn cached_confirmation_does_not_report_removed_client_or_retire_it_twice() {
        let mut clients = ClientRegistries::new();
        let first = clients.register(registration(1), &Credential::None).unwrap();
        assert_eq!(
            clients.confirm(first, &Credential::None).unwrap().clientid,
            Some(first.clientid)
        );
        let reboot = clients.register(registration(2), &Credential::None).unwrap();
        let confirmed = clients.confirm(reboot, &Credential::None).unwrap();
        assert_eq!(confirmed.clientid, Some(reboot.clientid));
        assert_eq!(confirmed.retired, Some(first.clientid));
        let retry = clients.confirm(reboot, &Credential::None).unwrap();
        assert_eq!(retry.clientid, Some(reboot.clientid));
        assert_eq!(retry.retired, None);
        assert_eq!(clients.invalidate(reboot.clientid), Some(reboot.clientid));
        let cached = clients.confirm(reboot, &Credential::None).unwrap();
        assert_eq!(cached.clientid, None);
        assert_eq!(cached.retired, None);
    }

    #[test]
    fn credentials_and_pending_invalidation_are_handled_by_the_owning_registry() {
        let mut clients = ClientRegistries::new();
        let unchecked = clients.register_unchecked(connection(), &Credential::None).unwrap();
        assert_eq!(clients.register_unchecked(connection(), &foreign()), Err(Error::IdentityInUse));
        let pending = clients.register(registration(1), &Credential::None).unwrap();
        assert_eq!(clients.confirm(pending, &foreign()).err(), Some(Error::IdentityInUse));
        assert_eq!(clients.invalidate(pending.clientid), None);
        assert_eq!(clients.confirm(pending, &Credential::None).err(), Some(Error::StaleClientId));
        assert_eq!(clients.lookup_unchecked(connection()), Some(unchecked));
        assert_eq!(clients.invalidate(ClientId::new(u64::MAX)), None);
        assert_eq!(clients.invalidate(unchecked), Some(unchecked));
    }
}
