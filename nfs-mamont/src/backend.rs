//! Runtime registry of [`crate::vfs::Vfs`] backends.
//!
//! The server starts without any backend and serves requests against the backends
//! attached later through [`BackendRegistry::add`]. Every backend is identified by a
//! [`BackendId`] that is encoded in the first byte of the file handles it hands out,
//! so an incoming procedure is routed by looking at
//! [`file::Handle::backend_id`](crate::vfs::file::Handle::backend_id).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::vfs::file::BackendId;

/// Shared, cheaply clonable map of attached backends.
///
/// The registry is stored behind an [`RwLock`] so backends can be added and removed
/// while the server is running: VFS workers only take the read lock to clone the
/// [`Arc`] of the backend addressed by the request.
pub struct BackendRegistry<V> {
    /// Attached backends, keyed by the index encoded in their file handles.
    backends: Arc<RwLock<HashMap<BackendId, Arc<V>>>>,
}

impl<V> BackendRegistry<V> {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self { backends: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Attaches `backend` under the lowest free index.
    ///
    /// # Returns
    ///
    /// The index the backend is registered under, or [`None`] if all
    /// [`BackendId::MAX`] + 1 slots are taken.
    pub fn add(&self, backend: Arc<V>) -> Option<BackendId> {
        let mut backends = self.write();
        let id = (BackendId::MIN..=BackendId::MAX).find(|id| !backends.contains_key(id))?;
        backends.insert(id, backend);

        Some(id)
    }

    /// Detaches the backend registered under `id`.
    ///
    /// In-flight procedures already dispatched to that backend run to completion;
    /// later requests carrying its handles fail with [`crate::vfs::Error::StaleFile`].
    ///
    /// # Returns
    ///
    /// The detached backend, or [`None`] if `id` was not registered.
    pub fn remove(&self, id: BackendId) -> Option<Arc<V>> {
        self.write().remove(&id)
    }

    /// Returns the backend registered under `id`, if any.
    pub fn get(&self, id: BackendId) -> Option<Arc<V>> {
        self.read().get(&id).map(Arc::clone)
    }

    /// Returns the indices of all attached backends, in ascending order.
    pub fn ids(&self) -> Vec<BackendId> {
        let mut ids = self.read().keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();

        ids
    }

    /// Returns the number of attached backends.
    pub fn len(&self) -> usize {
        self.read().len()
    }

    /// Returns `true` when no backend is attached.
    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }

    /// Takes the read lock, recovering the map if a previous holder panicked.
    fn read(&self) -> std::sync::RwLockReadGuard<'_, HashMap<BackendId, Arc<V>>> {
        self.backends.read().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Takes the write lock, recovering the map if a previous holder panicked.
    fn write(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<BackendId, Arc<V>>> {
        self.backends.write().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl<V> Clone for BackendRegistry<V> {
    /// Clones the shared view of the same registry, not the backends themselves.
    fn clone(&self) -> Self {
        Self { backends: Arc::clone(&self.backends) }
    }
}

impl<V> Default for BackendRegistry<V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::BackendRegistry;
    use std::sync::Arc;

    #[test]
    fn add_assigns_sequential_ids() {
        let registry = BackendRegistry::new();

        assert_eq!(registry.add(Arc::new("first")), Some(0));
        assert_eq!(registry.add(Arc::new("second")), Some(1));
        assert_eq!(registry.ids(), vec![0, 1]);
    }

    #[test]
    fn add_reuses_freed_id() {
        let registry = BackendRegistry::new();

        let first = registry.add(Arc::new("first")).unwrap();
        registry.add(Arc::new("second")).unwrap();

        assert_eq!(registry.remove(first).as_deref(), Some(&"first"));
        assert_eq!(registry.add(Arc::new("third")), Some(first));
    }

    #[test]
    fn remove_unknown_id_returns_none() {
        let registry: BackendRegistry<&str> = BackendRegistry::new();

        assert!(registry.is_empty());
        assert!(registry.remove(7).is_none());
        assert!(registry.get(7).is_none());
    }

    #[test]
    fn add_fails_when_all_slots_are_taken() {
        let registry = BackendRegistry::new();

        for _ in 0..=u8::MAX as usize {
            registry.add(Arc::new("backend")).unwrap();
        }

        assert_eq!(registry.len(), u8::MAX as usize + 1);
        assert_eq!(registry.add(Arc::new("overflow")), None);
    }

    #[test]
    fn clone_shares_the_same_map() {
        let registry = BackendRegistry::new();
        let clone = registry.clone();

        let id = clone.add(Arc::new("backend")).unwrap();

        assert_eq!(registry.get(id).as_deref(), Some(&"backend"));
    }
}
