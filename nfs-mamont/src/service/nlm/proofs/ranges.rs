//! Loop-free range arithmetic, proved for every `u64` input.

use super::{any_lock, contains, last_byte, overlap};
use crate::service::nlm::range_ops::{calculate_end_of_interval, ranges_overlap, split_lock};

/// The computed end of a range is its exact last byte.
#[kani::proof]
fn end_of_interval_is_exact() {
    let start: u64 = kani::any();
    let len: u64 = kani::any();
    assert_eq!(calculate_end_of_interval(start, len), last_byte(start, len));
}

/// Two ranges are reported as overlapping exactly when they share a byte.
#[kani::proof]
fn ranges_overlap_is_exact() {
    let (start1, len1, start2, len2) = (kani::any(), kani::any(), kani::any(), kani::any());
    assert_eq!(ranges_overlap(start1, len1, start2, len2), overlap(start1, len1, start2, len2));
}

#[kani::proof]
fn ranges_overlap_is_symmetric() {
    let (start1, len1, start2, len2) = (kani::any(), kani::any(), kani::any(), kani::any());
    assert_eq!(
        ranges_overlap(start1, len1, start2, len2),
        ranges_overlap(start2, len2, start1, len1)
    );
}

/// Every range overlaps itself, so identical locks of two owners always conflict.
#[kani::proof]
fn ranges_overlap_is_reflexive() {
    let start: u64 = kani::any();
    let len: u64 = kani::any();
    assert!(ranges_overlap(start, len, start, len));
}

/// Splitting a lock by an overlapping unlock removes exactly the unlocked
/// bytes and keeps owner and mode. It yields at most two fragments, and none
/// of them overlaps the unlocked range: `drain_overlapping` relies on that to
/// terminate.
#[kani::proof]
#[kani::unwind(3)]
fn split_lock_removes_exactly_the_unlocked_bytes() {
    let lock = any_lock();
    let unlock_start: u64 = kani::any();
    let unlock_len: u64 = kani::any();
    kani::assume(ranges_overlap(lock.offset, lock.length, unlock_start, unlock_len));

    let fragments = split_lock(lock.clone(), unlock_start, unlock_len).unwrap();
    assert!(fragments.len() <= 2);
    for fragment in &fragments {
        assert!(fragment.caller_name == lock.caller_name);
        assert!(fragment.system_identifier == lock.system_identifier);
        assert!(fragment.exclusive == lock.exclusive);
        assert!(!ranges_overlap(fragment.offset, fragment.length, unlock_start, unlock_len));
    }

    let x: u64 = kani::any();
    let kept = fragments.iter().any(|fragment| contains(fragment.offset, fragment.length, x));
    let expected = contains(lock.offset, lock.length, x) && !contains(unlock_start, unlock_len, x);
    assert_eq!(kept, expected);
}
