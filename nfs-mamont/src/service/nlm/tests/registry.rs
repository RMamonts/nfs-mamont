use super::super::ActiveLock;
use super::{fill_fh, make_active_lock, push_lock};
use crate::service::nlm::LockRegistry;

fn other_request() -> ActiveLock {
    make_active_lock("other", 999, false, 0, 0, 0)
}

fn same_owner_request() -> ActiveLock {
    make_active_lock("a", 1, true, 0, 100, 1)
}

#[test]
fn no_conflict_when_no_locks() {
    assert!(LockRegistry::new().find_conflict(&fill_fh(1), &other_request()).is_none());
}

#[test]
fn exclusive_conflicts_with_existing_exclusive() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("a", 1, true, 0, 100, 1));
    assert!(reg.find_conflict(&fill_fh(1), &other_request()).is_some());
}

#[test]
fn shared_does_not_conflict_with_shared() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("a", 1, false, 0, 100, 1));
    assert!(reg.find_conflict(&fill_fh(1), &other_request()).is_none());
}

#[test]
fn shared_conflicts_with_exclusive() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("a", 1, true, 0, 100, 1));
    assert!(reg.find_conflict(&fill_fh(1), &other_request()).is_some());
}

#[test]
fn no_conflict_on_different_file() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("a", 1, true, 0, 100, 1));
    assert!(reg.find_conflict(&fill_fh(2), &other_request()).is_none());
}

#[test]
fn no_conflict_when_ranges_dont_overlap() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("a", 1, true, 0, 10, 1));
    let req = make_active_lock("other", 999, true, 10, 10, 0);
    assert!(reg.find_conflict(&fill_fh(1), &req).is_none());
}

#[test]
fn same_owner_lock_does_not_conflict() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("a", 1, true, 0, 100, 1));
    assert!(reg.find_conflict(&fill_fh(1), &same_owner_request()).is_none());
}

#[test]
fn find_conflict_returns_holder_with_correct_fields() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("a", 42, true, 10, 20, 7));
    let holder = reg.find_conflict(&fill_fh(1), &other_request()).unwrap();
    assert!(holder.exclusive);
    assert_eq!(holder.system_identifier, 42);
    assert_eq!(holder.opaque_handle.as_bytes(), &[7; crate::consts::nlm::OPAQUE_HANDLE_SIZE]);
    assert_eq!(holder.lock_offset, 10);
    assert_eq!(holder.lock_length, 20);
}

#[test]
fn remove_by_owner_removes_matching_lock() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("alice", 100, true, 0, 50, 1));
    let target = make_active_lock("alice", 100, false, 0, 50, 0);
    reg.remove_by_owner(&fill_fh(1), &target);
    assert!(reg.by_file.is_empty());
}

#[test]
fn remove_by_owner_removes_only_different_owner() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("alice", 100, true, 0, 50, 1));
    push_lock(&mut reg, 1, make_active_lock("bob", 200, true, 60, 50, 2));
    let target = make_active_lock("alice", 100, false, 0, 50, 0);
    reg.remove_by_owner(&fill_fh(1), &target);
    assert_eq!(reg.by_file.get(&fill_fh(1)).unwrap().len(), 1);
    assert_eq!(reg.by_file.get(&fill_fh(1)).unwrap()[0].caller_name, "bob");
}

#[test]
fn remove_by_owner_removes_only_matching_range() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("Alice", 100, true, 0, 50, 1));
    push_lock(&mut reg, 1, make_active_lock("Alice", 100, true, 100, 50, 2));
    let target = make_active_lock("Alice", 100, false, 0, 50, 0);
    reg.remove_by_owner(&fill_fh(1), &target);
    assert_eq!(reg.by_file.get(&fill_fh(1)).unwrap().len(), 1);
    assert_eq!(reg.by_file.get(&fill_fh(1)).unwrap()[0].offset, 100);
}

#[test]
fn remove_by_owner_noop_on_nonexistent_file() {
    let target = make_active_lock("nobody", 0, false, 0, 0, 0);
    LockRegistry::new().remove_by_owner(&fill_fh(99), &target);
}

#[test]
fn remove_by_owner_cleans_up_empty_vec() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("a", 1, true, 0, 10, 1));
    let target = make_active_lock("a", 1, false, 0, 10, 0);
    reg.remove_by_owner(&fill_fh(1), &target);
    assert!(!reg.by_file.contains_key(&fill_fh(1)));
}

#[test]
fn remove_by_owner_noop_when_range_differs() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, 1, make_active_lock("a", 1, true, 0, 50, 1));
    let target = make_active_lock("a", 1, false, 100, 50, 0);
    reg.remove_by_owner(&fill_fh(1), &target);
    assert!(reg.by_file.contains_key(&fill_fh(1)));
    assert_eq!(reg.by_file.get(&fill_fh(1)).unwrap().len(), 1);
}
