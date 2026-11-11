use std::net::SocketAddr;

/// Result of a client registration operation.
pub type Result<T> = std::result::Result<T, Error>;

/// Maximum opaque logical identity length (RFC 3530 §2.1).
/// Used internally when accepting registrations; not re-exported.
pub const OPAQUE_LIMIT: usize = 1024;

/// Server-assigned shorthand, distinct from the client's logical identity.
/// Use [`Self::new`] and [`Self::into_inner`] at the wire boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(u64);

impl ClientId {
    /// Wraps a server-assigned numeric identifier.
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Consumes the wrapper and returns the server-assigned numeric identifier.
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

/// RPC network identifier for an NFSv4 callback.
/// Preserves the wire string verbatim, including implementation-specific netids.
/// The RPC transport adapter interprets support and validity; no transport list
/// or length limit is imposed by this protocol-neutral registration type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetId(String);

impl NetId {
    /// Wraps the RPC network identifier without copying or normalizing it.
    pub fn new(netid: String) -> Self {
        Self(netid)
    }

    /// Consumes the wrapper and returns the original RPC network identifier.
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// Logical client identity, kept in the representation supplied by its adapter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientIdentity(Vec<u8>);

impl ClientIdentity {
    /// Accepts the opaque client.id, at most NFS4_OPAQUE_LIMIT bytes.
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() > OPAQUE_LIMIT {
            return Err(Error::InvalidIdentity);
        }
        Ok(Self(bytes))
    }

    /// Returns the opaque client.id bytes.
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

/// Callback destination and RPC parameters, preserved for the transport adapter.
/// The manager does not interpret addresses or initiate callbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callback {
    pub netid: NetId,
    pub address: SocketAddr,
    pub program: u32,
    pub ident: u32,
}

/// Client incarnation verifier, distinct from the server confirmation challenge.
/// Transparent over the eight wire bytes, with no additional storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientVerifier([u8; 8]);

impl ClientVerifier {
    /// Wraps the client incarnation's eight wire bytes.
    pub fn new(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }
}

/// SETCLIENTID arguments {v, x, k}; credentials are supplied separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub identity: ClientIdentity,
    pub verifier: ClientVerifier,
    pub callback: Callback,
}

/// Server confirmation challenge, distinct from the client incarnation verifier.
/// Transparent over its eight wire bytes, with no additional storage.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfirmationVerifier([u8; 8]);

impl ConfirmationVerifier {
    /// Encodes the server's counter as an eight-byte confirmation challenge.
    pub fn new(value: u64) -> Self {
        Self(value.to_be_bytes())
    }

    /// Accepts the verifier bytes supplied by SETCLIENTID_CONFIRM.
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }
}

/// Pair returned to the client and subsequently passed to `confirm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingClient {
    pub clientid: ClientId,
    pub confirmation: ConfirmationVerifier,
}

/// Common errors; adapters map these to their wire protocol statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Opaque client.id exceeds 1024 bytes.
    InvalidIdentity,
    /// Missing registration or mismatching confirmation pair.
    StaleClientId,
    /// The RPC principal differs from that of the existing registration.
    IdentityInUse,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIdentity => f.write_str("client identity exceeds 1024 bytes"),
            Self::IdentityInUse => f.write_str("client identity belongs to another RPC principal"),
            Self::StaleClientId => f.write_str("stale client identifier or confirmation verifier"),
        }
    }
}

impl std::error::Error for Error {}
