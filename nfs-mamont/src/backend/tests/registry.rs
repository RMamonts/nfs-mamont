use std::sync::Arc;

use crate::backend::BackendRegistry;
use crate::vfs::file::BackendId;

#[test]
fn add_assigns_sequential_ids() {
    let registry = BackendRegistry::new();

    assert_eq!(registry.add(Arc::new("first")), Some(BackendId::new(0)));
    assert_eq!(registry.add(Arc::new("second")), Some(BackendId::new(1)));
    assert_eq!(registry.ids(), vec![BackendId::new(0), BackendId::new(1)]);
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
    assert!(registry.remove(BackendId::new(7)).is_none());
    assert!(registry.get(BackendId::new(7)).is_none());
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
