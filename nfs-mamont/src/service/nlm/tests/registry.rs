use super::super::ActiveLock;
use super::{fill_fh, make_active_lock, push_lock, FH_DEFAULT, FH_OTHER, LOCK_WHOLE_LENGTH};
use crate::service::nlm::LockRegistry;

fn other_request() -> ActiveLock {
    make_active_lock("other", 999, false, 0, 0, 0)
}

fn same_owner_request() -> ActiveLock {
    make_active_lock("a", 1, true, 0, LOCK_WHOLE_LENGTH, 1)
}

#[test]
fn no_conflict_when_no_locks() {
    assert!(LockRegistry::new().find_conflict(&fill_fh(FH_DEFAULT), &other_request()).is_none());
}

#[test]
fn exclusive_conflicts_with_existing_exclusive() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, LOCK_WHOLE_LENGTH, 1));
    assert!(reg.find_conflict(&fill_fh(FH_DEFAULT), &other_request()).is_some());
}

#[test]
fn shared_does_not_conflict_with_shared() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, false, 0, LOCK_WHOLE_LENGTH, 1));
    assert!(reg.find_conflict(&fill_fh(FH_DEFAULT), &other_request()).is_none());
}

#[test]
fn shared_conflicts_with_exclusive() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, LOCK_WHOLE_LENGTH, 1));
    assert!(reg.find_conflict(&fill_fh(FH_DEFAULT), &other_request()).is_some());
}

#[test]
fn no_conflict_on_different_file() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, LOCK_WHOLE_LENGTH, 1));
    assert!(reg.find_conflict(&fill_fh(FH_OTHER), &other_request()).is_none());
}

#[test]
fn no_conflict_when_ranges_dont_overlap() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, 10, 1));
    let req = make_active_lock("other", 999, true, 10, 10, 0);
    assert!(reg.find_conflict(&fill_fh(FH_DEFAULT), &req).is_none());
}

#[test]
fn same_owner_lock_does_not_conflict() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, LOCK_WHOLE_LENGTH, 1));
    assert!(reg.find_conflict(&fill_fh(FH_DEFAULT), &same_owner_request()).is_none());
}

#[test]
fn find_conflict_returns_holder_with_correct_fields() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 42, true, 10, 20, 7));
    let holder = reg.find_conflict(&fill_fh(FH_DEFAULT), &other_request()).unwrap();
    assert!(holder.exclusive);
    assert_eq!(holder.system_identifier, 42);
    assert_eq!(holder.opaque_handle.as_bytes(), &[7; crate::consts::nlm::OPAQUE_HANDLE_SIZE]);
    assert_eq!(holder.lock_offset, 10);
    assert_eq!(holder.lock_length, 20);
}

#[test]
fn remove_by_owner_removes_matching_lock() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("alice", 100, true, 0, 50, 1));
    let target = make_active_lock("alice", 100, false, 0, 50, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    assert!(reg.by_file.is_empty());
}

#[test]
fn remove_by_owner_removes_only_different_owner() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("alice", 100, true, 0, 50, 1));
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("bob", 200, true, 60, 50, 2));
    let target = make_active_lock("alice", 100, false, 0, 50, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    assert_eq!(reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap().len(), 1);
    assert_eq!(reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap()[0].caller_name, "bob");
}

#[test]
fn remove_by_owner_removes_only_matching_range() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("Alice", 100, true, 0, 50, 1));
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("Alice", 100, true, 100, 50, 2));
    let target = make_active_lock("Alice", 100, false, 0, 50, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    assert_eq!(reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap().len(), 1);
    assert_eq!(reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap()[0].offset, 100);
}

#[test]
fn remove_by_owner_noop_on_nonexistent_file() {
    let target = make_active_lock("nobody", 0, false, 0, 0, 0);
    let _ = LockRegistry::new().remove_by_owner(&fill_fh(99), &target);
}

#[test]
fn remove_by_owner_cleans_up_empty_vec() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, 10, 1));
    let target = make_active_lock("a", 1, false, 0, 10, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    assert!(!reg.by_file.contains_key(&fill_fh(FH_DEFAULT)));
}

#[test]
fn remove_by_owner_noop_when_range_differs() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, 50, 1));
    let target = make_active_lock("a", 1, false, 100, 50, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    assert!(reg.by_file.contains_key(&fill_fh(FH_DEFAULT)));
    assert_eq!(reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap().len(), 1);
}

#[test]
fn remove_by_owner_trims_left() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, 100, 1));
    let target = make_active_lock("a", 1, false, 0, 50, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].offset, 50);
    assert_eq!(locks[0].length, 50);
}

#[test]
fn push_or_replace_merges_adjacent() {
    let mut reg = LockRegistry::new();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 0, 50, 1)).unwrap();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 50, 50, 1)).unwrap();
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].offset, 0);
    assert_eq!(locks[0].length, 100);
}

#[test]
fn push_or_replace_merges_overlap_left() {
    let mut reg = LockRegistry::new();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 0, 50, 1)).unwrap();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 25, 50, 1)).unwrap();
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].offset, 0);
    assert_eq!(locks[0].length, 75);
}

#[test]
fn push_or_replace_merges_overlap_right() {
    let mut reg = LockRegistry::new();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 50, 50, 1)).unwrap();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 0, 75, 1)).unwrap();
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].offset, 0);
    assert_eq!(locks[0].length, 100);
}

#[test]
fn push_or_replace_merges_multiple_adjacent() {
    let mut reg = LockRegistry::new();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 0, 10, 1)).unwrap();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 10, 10, 1)).unwrap();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 20, 10, 1)).unwrap();
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].offset, 0);
    assert_eq!(locks[0].length, 30);
}

#[test]
fn push_or_replace_removes_covered_range() {
    let mut reg = LockRegistry::new();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 3, 2, 1)).unwrap();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 1, 9, 1)).unwrap();
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].offset, 1);
    assert_eq!(locks[0].length, 9);
}

#[test]
fn push_or_replace_does_not_merge_different_owner() {
    let mut reg = LockRegistry::new();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 0, 50, 1)).unwrap();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("b", 1, true, 50, 50, 1)).unwrap();
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 2);
}

#[test]
fn push_or_replace_does_not_merge_different_mode() {
    let mut reg = LockRegistry::new();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 0, 50, 1)).unwrap();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, false, 50, 50, 1)).unwrap();
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 2);
}

#[test]
fn push_or_replace_merges_to_eof() {
    let mut reg = LockRegistry::new();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 10, 0, 1)).unwrap();
    reg.push_or_replace(fill_fh(FH_DEFAULT), make_active_lock("a", 1, true, 5, 5, 1)).unwrap();
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].offset, 5);
    assert_eq!(locks[0].length, 0);
}

#[test]
fn remove_by_owner_trims_left_to_eof() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 100, 0, 1));
    let target = make_active_lock("a", 1, false, 0, 101, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].offset, 101);
    assert_eq!(locks[0].length, 0);
}

#[test]
fn remove_by_owner_trims_right() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, 100, 1));
    let target = make_active_lock("a", 1, false, 50, 50, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].offset, 0);
    assert_eq!(locks[0].length, 50);
}

#[test]
fn remove_by_owner_splits_middle() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, 100, 1));
    let target = make_active_lock("a", 1, false, 40, 20, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 2);
    let mut sorted: Vec<_> = locks.clone();
    sorted.sort_by_key(|l| l.offset);
    assert_eq!(sorted[0].offset, 0);
    assert_eq!(sorted[0].length, 40);
    assert_eq!(sorted[1].offset, 60);
    assert_eq!(sorted[1].length, 40);
}

#[test]
fn remove_by_owner_split_preserves_lock_mode() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, false, 0, 100, 1));
    let target = make_active_lock("a", 1, false, 30, 40, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 2);
    assert!(!locks[0].exclusive);
    assert!(!locks[1].exclusive);
}

#[test]
fn remove_by_owner_split_different_owner_kept() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, 100, 1));
    let target = make_active_lock("b", 1, false, 0, 50, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].offset, 0);
}

#[test]
fn remove_by_owner_unlock_fully_contains_lock() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 50, 50, 1));
    let target = make_active_lock("a", 1, false, 0, 200, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    assert!(!reg.by_file.contains_key(&fill_fh(FH_DEFAULT)));
}

#[test]
fn remove_by_owner_unlock_past_eof_trims_right() {
    let mut reg = LockRegistry::new();
    push_lock(&mut reg, FH_DEFAULT, make_active_lock("a", 1, true, 0, 100, 1));
    let target = make_active_lock("a", 1, false, 50, 1000, 0);
    let _ = reg.remove_by_owner(&fill_fh(FH_DEFAULT), &target);
    let locks = reg.by_file.get(&fill_fh(FH_DEFAULT)).unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].offset, 0);
    assert_eq!(locks[0].length, 50);
}
