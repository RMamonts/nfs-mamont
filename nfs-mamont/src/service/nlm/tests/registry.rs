use super::super::ActiveLock;
use super::{fill_fh, fill_opaque, push_lock};
use crate::service::nlm::LockRegistry;

#[test]
fn no_conflict_when_no_locks() {
    assert!(LockRegistry::new().find_conflict(&fill_fh(1), true, 0, 100).is_none());
}

#[test]
fn exclusive_conflicts_with_existing_exclusive() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, true, 0, 100);
    assert!(reg.find_conflict(&fill_fh(1), true, 10, 20).is_some());
}

#[test]
fn shared_does_not_conflict_with_shared() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, false, 0, 100);
    assert!(reg.find_conflict(&fill_fh(1), false, 10, 20).is_none());
}

#[test]
fn shared_conflicts_with_exclusive() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, true, 0, 100);
    assert!(reg.find_conflict(&fill_fh(1), false, 10, 20).is_some());
}

#[test]
fn no_conflict_on_different_file() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, true, 0, 100);
    assert!(reg.find_conflict(&fill_fh(2), true, 0, 100).is_none());
}

#[test]
fn no_conflict_when_ranges_dont_overlap() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, true, 0, 10);
    assert!(reg.find_conflict(&fill_fh(1), true, 10, 10).is_none());
}

#[test]
fn find_conflict_returns_holder_with_correct_fields() {
    let mut reg = LockRegistry::new();
    reg.by_file.entry(fill_fh(1)).or_default().push(ActiveLock {
        caller_name: "a".into(),
        system_identifier: 42,
        exclusive: true,
        offset: 10,
        length: 20,
        opaque_handle: fill_opaque(7),
    });
    let holder = reg.find_conflict(&fill_fh(1), true, 0, 100).unwrap();
    assert!(holder.exclusive);
    assert_eq!(holder.system_identifier, 42);
    assert_eq!(holder.opaque_handle.as_bytes(), &[7; crate::consts::nlm::OPAQUE_HANDLE_SIZE]);
    assert_eq!(holder.lock_offset, 10);
    assert_eq!(holder.lock_length, 20);
}

#[test]
fn remove_by_owner_removes_matching_lock() {
    let mut reg = LockRegistry::new();
    reg.by_file.entry(fill_fh(1)).or_default().push(ActiveLock {
        caller_name: "alice".into(),
        system_identifier: 100,
        exclusive: true,
        offset: 0,
        length: 50,
        opaque_handle: fill_opaque(1),
    });
    let target = ActiveLock {
        caller_name: "alice".into(),
        system_identifier: 100,
        exclusive: false,
        offset: 0,
        length: 50,
        opaque_handle: fill_opaque(0),
    };
    reg.remove_by_owner(&fill_fh(1), &target);
    assert!(reg.by_file.is_empty());
}

#[test]
fn remove_by_owner_removes_only_different_owner() {
    let mut reg = LockRegistry::new();
    let locks = reg.by_file.entry(fill_fh(1)).or_default();
    locks.push(ActiveLock {
        caller_name: "alice".into(),
        system_identifier: 100,
        exclusive: true,
        offset: 0,
        length: 50,
        opaque_handle: fill_opaque(1),
    });
    locks.push(ActiveLock {
        caller_name: "bob".into(),
        system_identifier: 200,
        exclusive: true,
        offset: 60,
        length: 50,
        opaque_handle: fill_opaque(2),
    });
    let target = ActiveLock {
        caller_name: "alice".into(),
        system_identifier: 100,
        exclusive: false,
        offset: 0,
        length: 50,
        opaque_handle: fill_opaque(0),
    };
    reg.remove_by_owner(&fill_fh(1), &target);
    assert_eq!(reg.by_file.get(&fill_fh(1)).unwrap().len(), 1);
    assert_eq!(reg.by_file.get(&fill_fh(1)).unwrap()[0].caller_name, "bob");
}

#[test]
fn remove_by_owner_removes_only_matching_range() {
    let mut reg = LockRegistry::new();
    let locks = reg.by_file.entry(fill_fh(1)).or_default();
    locks.push(ActiveLock {
        caller_name: "Alice".into(),
        system_identifier: 100,
        exclusive: true,
        offset: 0,
        length: 50,
        opaque_handle: fill_opaque(1),
    });
    locks.push(ActiveLock {
        caller_name: "Alice".into(),
        system_identifier: 100,
        exclusive: true,
        offset: 100,
        length: 50,
        opaque_handle: fill_opaque(2),
    });
    let target = ActiveLock {
        caller_name: "Alice".into(),
        system_identifier: 100,
        exclusive: false,
        offset: 0,
        length: 50,
        opaque_handle: fill_opaque(0),
    };
    reg.remove_by_owner(&fill_fh(1), &target);
    assert_eq!(reg.by_file.get(&fill_fh(1)).unwrap().len(), 1);
    assert_eq!(reg.by_file.get(&fill_fh(1)).unwrap()[0].offset, 100);
}

#[test]
fn remove_by_owner_noop_on_nonexistent_file() {
    let target = ActiveLock {
        caller_name: "nobody".into(),
        system_identifier: 0,
        exclusive: false,
        offset: 0,
        length: 0,
        opaque_handle: fill_opaque(0),
    };
    LockRegistry::new().remove_by_owner(&fill_fh(99), &target);
}

#[test]
fn remove_by_owner_cleans_up_empty_vec() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, true, 0, 10);
    let target = ActiveLock {
        caller_name: "a".into(),
        system_identifier: 1,
        exclusive: false,
        offset: 0,
        length: 10,
        opaque_handle: fill_opaque(0),
    };
    reg.remove_by_owner(&fill_fh(1), &target);
    assert!(!reg.by_file.contains_key(&fill_fh(1)));
}

#[test]
fn remove_by_owner_noop_when_range_differs() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, true, 0, 50);
    let target = ActiveLock {
        caller_name: "a".into(),
        system_identifier: 1,
        exclusive: false,
        offset: 100,
        length: 50,
        opaque_handle: fill_opaque(0),
    };
    reg.remove_by_owner(&fill_fh(1), &target);
    assert!(reg.by_file.contains_key(&fill_fh(1)));
    assert_eq!(reg.by_file.get(&fill_fh(1)).unwrap().len(), 1);
}
