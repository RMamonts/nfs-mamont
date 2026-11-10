//! Transport metadata, independent of logical client registrations.

use std::net::SocketAddr;

use super::generator::Generator;

/// TCP connection generation within one server instance. A reconnect gets a fresh value;
/// this is not a reliable client reboot detector (clients may use many sockets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionGeneration(u64);

impl ConnectionGeneration {
    /// Creates new instance of ConnectionGeneration with specified `id`
    pub fn new(id: u64) -> ConnectionGeneration {
        ConnectionGeneration(id)
    }

    /// Consumes the wrapper and returns the numeric connection generation.
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

/// Counter owned by the server and reused for every accepted connection.
/// A single accept loop accesses it mutably, so no global state or atomics are needed.
#[derive(Default)]
pub struct ConnectionGenerationGenerator {
    counter: Generator,
}

impl ConnectionGenerationGenerator {
    /// Creates a sequence whose first allocated generation is one.
    pub fn new() -> Self {
        Self { counter: Generator::new() }
    }

    /// Advances this generator and returns a connection generation.
    pub fn allocate(&mut self) -> ConnectionGeneration {
        ConnectionGeneration(self.counter.next())
    }
}

/// Observed metadata attached to every request read from an accepted TCP socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionInfo {
    pub peer_addr: SocketAddr,
    pub generation: ConnectionGeneration,
}

impl ConnectionInfo {
    /// Combines the observed peer address with a generation allocated by the server.
    pub fn new(peer_addr: SocketAddr, generation: ConnectionGeneration) -> Self {
        Self { peer_addr, generation }
    }
}
