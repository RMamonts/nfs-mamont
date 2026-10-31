use std::sync::Arc;

use crate::backend::BackendRegistry;
use crate::vfs::file::BackendId;

/// Fills every slot of `registry` and returns the ids in attach order.
fn fill<V>(registry: &BackendRegistry<V>, backend: &Arc<V>) -> Vec<BackendId> {
    (0..BackendId::SLOTS).map(|_| registry.add(Arc::clone(backend)).unwrap()).collect()
}

#[test]
fn add_assigns_sequential_ids() {
    let registry = BackendRegistry::new();

    assert_eq!(registry.add(Arc::new("first")), Some(BackendId::new(0, 0)));
    assert_eq!(registry.add(Arc::new("second")), Some(BackendId::new(1, 0)));
    assert_eq!(registry.ids(), vec![BackendId::new(0, 0), BackendId::new(1, 0)]);
}

#[test]
fn freed_slot_goes_to_back_of_queue() {
    let registry = BackendRegistry::new();

    let first = registry.add(Arc::new("first")).unwrap();
    registry.add(Arc::new("second")).unwrap();

    assert_eq!(registry.remove(first).as_deref(), Some(&"first"));
    assert_eq!(registry.add(Arc::new("third")), Some(BackendId::new(2, 0)));
}

#[test]
fn readded_slot_gets_new_generation() {
    let registry = BackendRegistry::new();
    fill(&registry, &Arc::new("old"));

    let stale = BackendId::new(1, 0);
    assert_eq!(registry.remove(stale).as_deref(), Some(&"old"));

    let fresh = registry.add(Arc::new("new")).unwrap();
    assert_eq!(fresh, BackendId::new(1, 1));
    assert!(registry.get(stale).is_none());
    assert_eq!(registry.get(fresh).as_deref(), Some(&"new"));
    assert_eq!(registry.ids().len(), BackendId::SLOTS);
    assert!(registry.ids().contains(&fresh));
}

#[test]
fn remove_with_stale_generation_is_noop() {
    let registry = BackendRegistry::new();
    fill(&registry, &Arc::new("old"));

    let stale = BackendId::new(0, 0);
    registry.remove(stale).unwrap();
    let fresh = registry.add(Arc::new("new")).unwrap();

    assert!(registry.remove(stale).is_none());
    assert_eq!(registry.get(fresh).as_deref(), Some(&"new"));
    assert_eq!(registry.len(), BackendId::SLOTS);
}

#[test]
fn generation_wraps_after_256_reuses() {
    let registry = BackendRegistry::new();
    fill(&registry, &Arc::new("backend"));

    let mut id = BackendId::new(0, 0);
    for _ in 0..BackendId::SLOTS {
        registry.remove(id).unwrap();
        id = registry.add(Arc::new("backend")).unwrap();
        assert_eq!(id.slot(), 0);
    }

    assert_eq!(id, BackendId::new(0, 0));
}

#[test]
fn remove_unknown_id_returns_none() {
    let registry: BackendRegistry<&str> = BackendRegistry::new();

    assert!(registry.is_empty());
    assert!(registry.remove(BackendId::new(7, 0)).is_none());
    assert!(registry.get(BackendId::new(7, 0)).is_none());
}

#[test]
fn add_fails_when_all_slots_are_taken() {
    let registry = BackendRegistry::new();
    fill(&registry, &Arc::new("backend"));

    assert_eq!(registry.len(), BackendId::SLOTS);
    assert_eq!(registry.add(Arc::new("overflow")), None);
}

#[test]
fn clone_shares_the_same_map() {
    let registry = BackendRegistry::new();
    let clone = registry.clone();

    let id = clone.add(Arc::new("backend")).unwrap();

    assert_eq!(registry.get(id).as_deref(), Some(&"backend"));
}
