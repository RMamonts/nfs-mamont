//! Both protocol registries share one ID sequence and expose one removal API.

use crate::auth::Credential;
use crate::client_id::{
    ClientId, ClientIdGenerator, ClientIdManager, ClientIdentity, ConnectionInfo,
    Nfs3ClientIdManager, PendingClient, Registration, Result,
};

/// NFSv4 confirmation may replay a cached success after the registration was removed.
/// Only a currently confirmed client is returned for lease creation or renewal.
pub struct Confirmation {
    pub clientid: Option<ClientId>,
    pub retired: Option<ClientId>,
}

/// The registration path determines which registry owns a client. Managers and
/// their shared allocator remain private so callers cannot mix ownership.
///
/// A registered clientid belongs to exactly one manager: NFSv3 or NFSv4.
/// The same clientid must never exist in both managers. Both managers use a
/// shared ID sequence, and each registration method writes to only one of them.
pub struct ClientRegistries {
    nfsv4: ClientIdManager,
    nfsv3: Nfs3ClientIdManager,
}

impl ClientRegistries {
    /// Creates empty NFSv3 and NFSv4 registries sharing one clientid allocator.
    pub fn new() -> Self {
        let ids = ClientIdGenerator::new();
        Self { nfsv4: ClientIdManager::new(ids.clone()), nfsv3: Nfs3ClientIdManager::new(ids) }
    }

    /// Finds a confirmed NFSv4 client by its logical identity.
    pub fn lookup(&self, identity: &ClientIdentity) -> Option<ClientId> {
        self.nfsv4.lookup(identity)
    }

    /// Finds an NFSv3 client by the connection's peer address.
    pub fn lookup_unchecked(&self, connection: ConnectionInfo) -> Option<ClientId> {
        self.nfsv3.lookup(connection)
    }

    /// Registers an NFSv4 client that must subsequently be confirmed.
    pub fn register(
        &mut self,
        registration: Registration,
        credentials: &Credential,
    ) -> Result<PendingClient> {
        self.nfsv4.register(registration, credentials)
    }

    /// Registers an NFSv3 peer immediately, retaining credential collision checks.
    pub fn register_unchecked(
        &mut self,
        connection: ConnectionInfo,
        credentials: &Credential,
    ) -> Result<ClientId> {
        self.nfsv3.register(connection, credentials)
    }

    /// Confirms an NFSv4 registration and reports its current and retired IDs.
    /// A cached success may have no current ID if the registration was removed.
    pub fn confirm(
        &mut self,
        pair: PendingClient,
        credentials: &Credential,
    ) -> Result<Confirmation> {
        let retired = self.nfsv4.confirm(pair, credentials)?;
        let clientid = self.nfsv4.get(pair.clientid).ok().map(|_| pair.clientid);
        Ok(Confirmation { clientid, retired })
    }

    /// Removes a client from its owning registry only.
    ///
    /// Returns an active ID for state cleanup, or `None` for an unknown clientid,
    /// that does not appear neither in nfsv3 register, nor in nfsv4 register
    #[must_use]
    pub fn invalidate(&mut self, clientid: ClientId) -> Option<ClientId> {
        if self.nfsv3.get(clientid).is_ok() {
            self.nfsv3.invalidate(clientid).map(|_| clientid)
        } else {
            self.nfsv4.invalidate(clientid)
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
