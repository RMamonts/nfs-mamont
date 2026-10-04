//! Bounded harnesses for the lock-list operations behind LOCK and UNLOCK.
//!
//! Lists hold two or three locks with concrete lengths, so the sort inside
//! `merge_adjacent` unrolls to a fixed depth.

use super::{any_locks, any_owner, contains, holds, touch};
use crate::service::nlm::range_ops::{drain_overlapping, merge_adjacent};

/// Checks `merge_adjacent` on `N` arbitrary locks: it does not panic, every
/// owner keeps exactly its bytes in every mode, and no two locks of one owner
/// and mode remain that overlap or touch.
fn check_merge<const N: usize>() {
    let mut locks = any_locks::<N>();
    let before = locks.clone();
    merge_adjacent(&mut locks);

    let owner = any_owner();
    let exclusive: bool = kani::any();
    let x: u64 = kani::any();
    assert_eq!(holds(&locks, owner, exclusive, x), holds(&before, owner, exclusive, x));

    for (index, first) in locks.iter().enumerate() {
        for second in &locks[index + 1..] {
            let same_owner_and_mode = first.caller_name == second.caller_name
                && first.system_identifier == second.system_identifier
                && first.exclusive == second.exclusive;
            if same_owner_and_mode {
                assert!(!touch(first.offset, first.length, second.offset, second.length));
            }
        }
    }
}

#[kani::proof]
#[kani::unwind(4)]
fn merge_adjacent_two_locks() {
    check_merge::<2>();
}

#[kani::proof]
#[kani::unwind(5)]
fn merge_adjacent_three_locks() {
    check_merge::<3>();
}

/// Checks `drain_overlapping` on `N` arbitrary locks: the target owner loses
/// exactly the drained bytes in both modes, and every other owner keeps all
/// of its bytes.
fn check_drain<const N: usize>() {
    let mut locks = any_locks::<N>();
    let before = locks.clone();
    let target = any_owner();
    let start: u64 = kani::any();
    let len: u64 = kani::any();
    drain_overlapping(&mut locks, target.0, target.1, start, len).unwrap();

    let owner = any_owner();
    let exclusive: bool = kani::any();
    let x: u64 = kani::any();
    let drained = owner == target && contains(start, len, x);
    assert_eq!(holds(&locks, owner, exclusive, x), holds(&before, owner, exclusive, x) && !drained);
}

/// Every drained lock can be split into two fragments, so the loop runs at
/// most `3 * N` times.
#[kani::proof]
#[kani::unwind(8)]
fn drain_overlapping_two_locks() {
    check_drain::<2>();
}

#[kani::proof]
#[kani::unwind(11)]
fn drain_overlapping_three_locks() {
    check_drain::<3>();
}
