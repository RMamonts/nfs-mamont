use std::net::{IpAddr, SocketAddr};

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

/// Stable client host name, independent of protocol-specific name wrappers.
/// The name must be nonempty and at most 1024 bytes, matching the NLM caller-name
/// limit. Case and spelling are preserved; no DNS normalization is performed.
#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostName(String);

impl HostName {
    /// Validates the host name's byte length without copying its allocation.
    pub fn new(name: String) -> Result<Self> {
        const MAX_LEN: usize = 1024;
        if name.is_empty() || name.len() > MAX_LEN {
            return Err(Error::InvalidHostName);
        }
        Ok(Self(name))
    }

    /// Consumes the wrapper and returns the original host name.
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// RPC network identifier, distinct from a client host name.
/// Preserves the wire string verbatim, including implementation-specific netids.
/// The RPC transport adapter interprets support and validity; no transport list
/// or length limit is imposed by this protocol-neutral registration type.
#[repr(transparent)]
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
pub enum ClientIdentity {
    /// Client-supplied opaque identity from an explicit handshake (up to 1024 bytes).
    Handshake(Vec<u8>),
    /// Observed host address and client host name, without binary serialization.
    /// The host name is validated on construction; adapters canonicalize the address.
    Implicit { address: IpAddr, host_name: HostName },
}

/// Callback destination and RPC parameters, preserved for the transport adapter.
/// The manager does not interpret addresses or initiate callbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Callback {
    /// Advertised callback endpoint, RPC netid, program and callback identifier.
    /// The protocol adapter converts a wire universal address to `SocketAddr`.
    Universal { netid: NetId, address: SocketAddr, program: u32, ident: u32 },
    /// Observed peer and callback program. Resolve the service port before use.
    Peer { address: SocketAddr, program: u32 },
}

/// Client incarnation verifier, distinct from the server confirmation challenge.
/// Transparent over the eight wire bytes, with no additional storage.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientVerifier([u8; 8]);

impl ClientVerifier {
    /// Wraps the client incarnation's eight wire bytes.
    pub fn new(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }
}

/// Common client registration data produced by protocol adapters.
/// Authentication and authorization belong to the calling RPC layer.
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
    /// Wraps the server confirmation challenge's eight wire bytes.
    pub fn new(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }
}

/// Pair returned to the client and subsequently passed to `confirm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingClient {
    pub clientid: ClientId,
    pub confirmation: ConfirmationVerifier,
}

/// RPC retransmission identity. Scope must uniquely identify a transport/session
/// within the server lifetime. Exact operation arguments are also checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestId {
    pub scope: u64,
    pub xid: u32,
}

/// Successful confirmation/implicit insertion, possibly replacing an incarnation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "apply any retired client cleanup before using the replacement"]
pub struct Transition {
    pub clientid: ClientId,
    /// Previous confirmed ID; its state is handled by the external state/lease layer.
    pub retired: Option<ClientId>,
}

/// Common errors; adapters map these to their wire protocol statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Opaque identity exceeds 1024 bytes, or uses the wrong registration domain.
    InvalidIdentity,
    /// Client host name is empty or exceeds 1024 bytes.
    InvalidHostName,
    /// Missing registration or mismatching confirmation pair.
    StaleClientId,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIdentity => f.write_str("invalid client identity length or domain"),
            Self::InvalidHostName => f.write_str("client host name must contain 1 to 1024 bytes"),
            Self::StaleClientId => f.write_str("stale client identifier or confirmation verifier"),
        }
    }
}

impl std::error::Error for Error {}
