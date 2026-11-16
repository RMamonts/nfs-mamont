//! Defines the NFSv4 SETCLIENTID operation types, RFC 7530 §16.33.

use crate::vfs::nfsv4::file;

/// A network address of the client (`clientaddr4`), RFC 7530 §2.2.10.
/// TODO: remove after <https://github.com/RMamonts/nfs-mamont/pull/461>
pub struct ClientAddr {
    /// Network id of the transport (`r_netid`).
    pub r_netid: String,
    /// Universal address of the transport (`r_addr`).
    pub r_addr: String,
}

/// The callback registration of the client (`cb_client4`), RFC 7530 §2.2.11.
/// TODO: remove after <https://github.com/RMamonts/nfs-mamont/pull/461>
pub struct Callback {
    /// Program number of the callback RPC service.
    pub cb_program: u32,
    /// Address at which the callback service is located.
    pub cb_location: ClientAddr,
}

/// The identifier supplied by the client (`nfs_client_id4`), RFC 7530 §2.2.12.
/// TODO: remove after <https://github.com/RMamonts/nfs-mamont/pull/461>
pub struct Client {
    /// Verifier of the client identity.
    pub verifier: super::Verifier,
    /// Opaque identifier of the client, at most `NFS4_OPAQUE_LIMIT` bytes.
    pub id: Vec<u8>,
}

/// SETCLIENTID arguments (`SETCLIENTID4args`).
pub struct Args {
    /// Client identity sent by the client.
    pub client: Client,
    /// Callback registration information.
    pub callback: Callback,
    /// Identifier used to distinguish multiple servers on a single callback
    /// program.
    pub callback_ident: u32,
}

/// Success result (`SETCLIENTID4resok`).
pub struct Success {
    /// Shorthand client id assigned by the server.
    pub clientid: file::ClientId,
    /// Verifier to be used to confirm the client id.
    pub confirm: super::Verifier,
}

/// Fail result (`SETCLIENTID4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
    /// Address of the client using the conflicting client id, present when
    /// the error is `NFS4ERR_CLID_INUSE`.
    pub client_using: Option<ClientAddr>,
}
