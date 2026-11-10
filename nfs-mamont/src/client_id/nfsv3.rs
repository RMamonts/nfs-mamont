//! Explicit NFSv3 connection registration for locking and callback routing.
//!
//! The caller supplies transport metadata and validated RPC credentials. An
//! accepted TCP socket alone does not provide credentials: register once the
//! caller identity is available. This module does not modify the accept loop or
//! NLM handlers.
//!
//! A registration identifies a connection. `caller_name`, `opaque_handle` and
//! `system_identifier` from NLM locks identify lock owners associated with that
//! connection; they are not needed to allocate its clientid.
//! Registration uses the peer address and credentials. Connection generations are
//! transport metadata only; they never act as a verifier or trigger replacement.
//! The caller explicitly invalidates a registration when its lifetime ends.
//!
//! The observed peer address supplies a callback route. Its port is the incoming
//! connection's source port, not an advertised NLM callback service port. Resolve
//! the callback service separately before sending callbacks.
//!
//! IDs belong to the registry that issued them. Shared lock storage must retain
//! the protocol/registry context alongside a clientid. Cleanup of locks and
//! pending callbacks must be coordinated with invalidation by the caller.
//!
//! ```
//! use nfs_mamont::{auth::Credential, client_id::*};
//!
//! let mut generations = ConnectionGenerationGenerator::new();
//! let connection = ConnectionInfo::new(
//!     "192.0.2.1:40000".parse().unwrap(), generations.allocate(),
//! );
//! let mut clients = Nfs3ClientIdManager::new();
//! let clientid = clients.register(connection, &Credential::None).unwrap();
//! assert_eq!(clients.get(clientid).unwrap().connection, connection);
//! assert_eq!(clients.lookup(connection), Some(clientid));
//! ```

use std::collections::HashMap;
use std::net::IpAddr;

use crate::auth::Credential;

use super::{
    connection::ConnectionInfo,
    generator::Generator,
    types::{ClientId, Error, Result},
};

/// Registered NLM connection and its caller identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nfs3Client {
    pub connection: ConnectionInfo,
    pub credentials: Credential,
}

/// Explicit NLM registrations, immediately usable without confirmation or DRC.
/// Peer addresses identify registrations; no verifier is required or compared.
/// The caller explicitly removes records when their lifetime ends.
#[derive(Default)]
pub struct Nfs3ClientIdManager {
    ids: Generator,
    clients: HashMap<ClientId, Nfs3Client>,
}

impl Nfs3ClientIdManager {
    /// Creates an empty connection registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the connection immediately. Repeating a registration preserves
    /// its ID, including when connection generation changes. Registering the
    /// same peer address with different credentials is a collision and preserves
    /// the existing registration. Generation is retained as transport metadata.
    pub fn register(
        &mut self,
        connection: ConnectionInfo,
        credentials: &Credential,
    ) -> Result<ClientId> {
        if let Some((clientid, client)) = self
            .clients
            .iter()
            .find(|(_, client)| client.connection.peer_addr == connection.peer_addr)
        {
            if client.credentials != *credentials {
                return Err(Error::IdentityInUse);
            }
            return Ok(*clientid);
        }
        let clientid = ClientId::new(self.ids.next());
        self.clients.insert(clientid, Nfs3Client { connection, credentials: credentials.clone() });
        Ok(clientid)
    }

    /// Returns a registered connection and its callback routing information.
    pub fn get(&self, clientid: ClientId) -> Result<&Nfs3Client> {
        let Some(client) = self.clients.get(&clientid) else {
            return Err(Error::StaleClientId);
        };
        Ok(client)
    }

    /// Returns the registered peer's host for NLM callback service resolution.
    /// The incoming TCP source port is not the callback service port.
    pub fn callback_address(&self, clientid: ClientId) -> Result<IpAddr> {
        Ok(self.get(clientid)?.connection.peer_addr.ip())
    }

    /// Finds an existing registration without creating one or changing its owner.
    pub fn lookup(&self, connection: ConnectionInfo) -> Option<ClientId> {
        self.clients
            .iter()
            .find(|(_, client)| client.connection.peer_addr == connection.peer_addr)
            .map(|(id, _)| *id)
    }

    /// Removes a registration and returns it for lock/callback cleanup. Other
    /// peer addresses are preserved.
    #[must_use = "the caller must clean up locks and callbacks for the removed registration"]
    pub fn invalidate(&mut self, clientid: ClientId) -> Option<Nfs3Client> {
        self.clients.remove(&clientid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auth::AuthSysParams, client_id::ConnectionGenerationGenerator};

    fn credential(uid: u32) -> Credential {
        Credential::Sys(AuthSysParams {
            stamp: 1,
            machine_name: "host".into(),
            uid,
            gid: 10,
            gids: vec![],
        })
    }

    #[test]
    fn registration_is_explicit_immediate_and_repeatable() {
        let mut manager = Nfs3ClientIdManager::new();
        let mut generations = ConnectionGenerationGenerator::new();
        let connection =
            ConnectionInfo::new("192.0.2.1:40000".parse().unwrap(), generations.allocate());
        let owner = credential(100);
        assert_eq!(manager.lookup(connection), None);
        let id = manager.register(connection, &owner).unwrap();
        assert_eq!(manager.register(connection, &owner).unwrap(), id);
        assert_eq!(manager.lookup(connection), Some(id));
        assert_eq!(manager.callback_address(id), Ok(connection.peer_addr.ip()));
        assert_eq!(manager.get(id).unwrap(), &Nfs3Client { connection, credentials: owner });
    }

    #[test]
    fn credential_collisions_preserve_registration() {
        let mut manager = Nfs3ClientIdManager::new();
        let mut generations = ConnectionGenerationGenerator::new();
        let connection =
            ConnectionInfo::new("192.0.2.1:40000".parse().unwrap(), generations.allocate());
        let owner = credential(100);
        let id = manager.register(connection, &owner).unwrap();
        assert_eq!(manager.register(connection, &credential(200)), Err(Error::IdentityInUse));
        assert_eq!(manager.register(connection, &Credential::None), Err(Error::IdentityInUse));
        let different_peer =
            ConnectionInfo::new("192.0.2.2:40000".parse().unwrap(), connection.generation);
        let other_id = manager.register(different_peer, &owner).unwrap();
        assert_ne!(id, other_id);
        assert_eq!(manager.get(id).unwrap(), &Nfs3Client { connection, credentials: owner });
    }

    #[test]
    fn generations_are_not_verifiers_and_other_peers_are_independent() {
        let mut manager = Nfs3ClientIdManager::new();
        let mut generations = ConnectionGenerationGenerator::new();
        let address = "192.0.2.1:40000".parse().unwrap();
        let first = ConnectionInfo::new(address, generations.allocate());
        let second = ConnectionInfo::new(address, generations.allocate());
        let old = manager.register(first, &credential(100)).unwrap();
        assert_eq!(manager.register(second, &credential(100)).unwrap(), old);
        assert_eq!(manager.lookup(second), Some(old));
        assert_eq!(manager.register(second, &credential(200)), Err(Error::IdentityInUse));
        let other = ConnectionInfo::new("192.0.2.1:40001".parse().unwrap(), generations.allocate());
        let new = manager.register(other, &credential(200)).unwrap();
        assert_ne!(old, new);
        let removed = manager.invalidate(old).unwrap();
        assert_eq!(removed.connection, first);
        assert_eq!(manager.get(old), Err(Error::StaleClientId));
        assert_eq!(manager.callback_address(old), Err(Error::StaleClientId));
        assert_eq!(manager.lookup(first), None);
        assert_eq!(manager.lookup(second), None);
        assert_eq!(manager.lookup(other), Some(new));
        assert!(manager.get(new).is_ok());
        assert_eq!(manager.invalidate(old), None);
    }

    #[test]
    fn invalidated_connection_gets_fresh_id_on_explicit_registration() {
        let mut manager = Nfs3ClientIdManager::new();
        let mut generations = ConnectionGenerationGenerator::new();
        let connection =
            ConnectionInfo::new("192.0.2.1:40000".parse().unwrap(), generations.allocate());
        let old = manager.register(connection, &Credential::None).unwrap();
        let _ = manager.invalidate(old).unwrap();
        let new = manager.register(connection, &Credential::None).unwrap();
        assert_ne!(old, new);
        assert_eq!(manager.get(old), Err(Error::StaleClientId));
        assert_eq!(manager.lookup(connection), Some(new));
    }
}
