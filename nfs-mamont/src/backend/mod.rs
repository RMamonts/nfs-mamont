//! Runtime registry of [`crate::vfs::Vfs`] backends.
//!
//! The server starts without any backend and serves requests against the backends
//! attached later through [`BackendRegistry::add`]. Every backend is identified by a
//! [`BackendId`] that is encoded in the first two bytes of the file handles it hands
//! out, so an incoming procedure is routed by looking at
//! [`file::Handle::backend_id`](crate::vfs::file::Handle::backend_id).
//!
//! A [`BackendId`] is a registry slot plus the generation of that slot. The generation
//! is bumped whenever a backend is detached, so handles of a detached backend turn
//! stale instead of resolving to the next backend attached to the same slot. Freed
//! slots are reused in FIFO order, which pushes a generation wrap-around on any single
//! slot as far away as possible.

#[cfg(test)]
mod tests;

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

use crate::vfs::file::BackendId;

/// One registry slot.
struct Slot<V> {
    /// Generation handed out to the next backend attached to this slot, or the
    /// generation of the backend currently attached.
    generation: u8,
    /// The attached backend, if any.
    backend: Option<Arc<V>>,
}

/// Fixed table of [`BackendId::SLOTS`] slots plus the queue of free ones.
struct Slots<V> {
    /// Slots indexed by [`BackendId::slot`].
    entries: Box<[Slot<V>]>,
    /// Free slots, the least recently freed first.
    free: VecDeque<u8>,
}

impl<V> Slots<V> {
    fn new() -> Self {
        let entries =
            (0..BackendId::SLOTS).map(|_| Slot { generation: 0, backend: None }).collect();
        let free = (0..=u8::MAX).collect();

        Self { entries, free }
    }
}

/// Shared, cheaply clonable table of attached backends.
///
/// The registry is stored behind an [`RwLock`] so backends can be added and removed
/// while the server is running: VFS workers only take the read lock to clone the
/// [`Arc`] of the backend addressed by the request.
pub struct BackendRegistry<V> {
    /// Attached backends, indexed by the slot encoded in their file handles.
    slots: Arc<RwLock<Slots<V>>>,
}

impl<V> BackendRegistry<V> {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self { slots: Arc::new(RwLock::new(Slots::new())) }
    }

    /// Attaches `backend` under the least recently freed slot.
    ///
    /// # Returns
    ///
    /// The id the backend is registered under, or [`None`] if all
    /// [`BackendId::SLOTS`] slots are taken.
    pub fn add(&self, backend: Arc<V>) -> Option<BackendId> {
        let mut slots = self.slots.write().unwrap();
        let index = slots.free.pop_front()?;
        let slot = &mut slots.entries[usize::from(index)];
        slot.backend = Some(backend);

        Some(BackendId::new(index, slot.generation))
    }

    /// Detaches the backend registered under `id`.
    ///
    /// In-flight procedures already dispatched to that backend run to completion;
    /// later requests carrying its handles fail with [`crate::vfs::Error::StaleFile`],
    /// even after another backend is attached to the same slot.
    ///
    /// # Returns
    ///
    /// The detached backend, or [`None`] if `id` was not registered.
    pub fn remove(&self, id: BackendId) -> Option<Arc<V>> {
        let mut guard = self.slots.write().unwrap();
        let slots = &mut *guard;
        let slot = &mut slots.entries[usize::from(id.slot())];
        if slot.generation != id.generation() {
            return None;
        }
        let backend = slot.backend.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        slots.free.push_back(id.slot());

        Some(backend)
    }

    /// Returns the backend registered under `id`, if any.
    pub fn get(&self, id: BackendId) -> Option<Arc<V>> {
        let slots = self.slots.read().unwrap();
        let slot = &slots.entries[usize::from(id.slot())];
        if slot.generation != id.generation() {
            return None;
        }

        slot.backend.clone()
    }

    /// Returns the ids of all attached backends, in ascending slot order.
    pub fn ids(&self) -> Vec<BackendId> {
        let slots = self.slots.read().unwrap();

        (0..=u8::MAX)
            .zip(slots.entries.iter())
            .filter(|(_, slot)| slot.backend.is_some())
            .map(|(index, slot)| BackendId::new(index, slot.generation))
            .collect()
    }

    /// Returns the number of attached backends.
    pub fn len(&self) -> usize {
        BackendId::SLOTS - self.slots.read().unwrap().free.len()
    }

    /// Returns `true` when no backend is attached.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<V> Clone for BackendRegistry<V> {
    /// Clones the shared view of the same registry, not the backends themselves.
    fn clone(&self) -> Self {
        Self { slots: Arc::clone(&self.slots) }
    }
}

impl<V> Default for BackendRegistry<V> {
    fn default() -> Self {
        Self::new()
    }
}
