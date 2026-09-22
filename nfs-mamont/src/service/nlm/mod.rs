//! Server-side state and handlers for the NLM v4 RPC program.
//!
//! This module provides an in-memory lock manager that tracks active locks
//! grouped by file handle. The registry supports shared/exclusive semantics
//! and range-based conflict detection.
//!
//! The service implements `Lock`, `Unlock`, `Test` and `Cancel`
//! procedure traits from `crate::nlm::procedures`.

use crate::service::nlm::lock_registry::LockRegistry;

mod arithmetic;
mod lock_registry;
mod lock_types;

mod procedures;
#[cfg(test)]
mod tests;

/// In-memory state backing the NLM v4 service implementation.
///
/// Holds a lock registry protected by a read-write lock so that
/// multiple `TEST` (read-only) requests can proceed concurrently
/// while `LOCK`/`UNLOCK`/`CANCEL` (write) requests are serialised.
pub struct NlmService {
    /// Active locks grouped by file handle.
    locks: tokio::sync::RwLock<LockRegistry>,
}

impl Default for NlmService {
    /// Creates an empty [`NlmService`] with no locks registered.
    fn default() -> Self {
        NlmService { locks: tokio::sync::RwLock::new(LockRegistry::new()) }
    }
}

impl NlmService {
    /// Creates an empty [`NlmService`].
    pub fn new() -> Self {
        Self::default()
    }
}
