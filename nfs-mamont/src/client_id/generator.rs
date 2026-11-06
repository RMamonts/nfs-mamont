use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use super::types::ClientId;

/// An internal incrementing 64-bit sequence, independent of registrations.
#[derive(Default)]
pub struct Generator {
    last: u64,
}

/// Clientid allocator shared by registries belonging to the same server.
/// Cloning the allocator shares its counter rather than copying its state.
#[derive(Clone, Default)]
pub struct ClientIdGenerator {
    counter: Arc<AtomicU64>,
}

impl ClientIdGenerator {
    /// Creates an empty allocation sequence.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates a fresh clientid for either protocol registry.
    pub fn allocate(&self) -> ClientId {
        ClientId::new(self.counter.fetch_add(1, Ordering::Relaxed).wrapping_add(1))
    }
}

impl Generator {
    pub const fn new() -> Self {
        Self { last: 0 }
    }

    pub fn next(&mut self) -> u64 {
        self.last = self.last.wrapping_add(1);
        self.last
    }
}
