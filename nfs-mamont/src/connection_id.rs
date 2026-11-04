use std::net::SocketAddr;

/// The connection's ID at the server level. Used in mount and nlm.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct ConnectionId(SocketAddr);

impl ConnectionId {
    /// Creates a new connection ID based on the client's IP address and port.
    pub fn new(addr: SocketAddr) -> Self {
        ConnectionId(addr)
    }

    /// Consumes the wrapper and returns the inner [`SocketAddr`].
    pub fn into_inner(self) -> SocketAddr {
        self.0
    }
}