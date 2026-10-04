//! Kani harnesses for the NLM byte-range arithmetic.
//!
//! Byte coverage is compared against an exact model ([`last_byte`]) rather
//! than against [`calculate_end_of_interval`], so saturation mistakes at the
//! end of the `u64` space are reported instead of mirrored.
//!
//! [`calculate_end_of_interval`]: super::range_ops::calculate_end_of_interval

mod merge;
mod ranges;

use crate::nlm::OpaqueHandle;
use crate::service::nlm::lock_types::ActiveLock;

/// Lock owner identity: `(caller_name, system_identifier)`.
type Owner = (&'static str, i32);

/// Owners used by the harnesses: two are enough to tell same-owner merging
/// from cross-owner conflicts.
const OWNERS: [Owner; 2] = [("a", 1), ("b", 2)];

/// Last byte of `[start, start + len)` in exact arithmetic, clipped to the
/// last addressable byte; `len == 0` means "to end of file".
fn last_byte(start: u64, len: u64) -> u64 {
    if len == 0 {
        return u64::MAX;
    }
    let last = start as u128 + len as u128 - 1;
    last.min(u64::MAX as u128) as u64
}

/// Whether byte `x` lies in `[start, start + len)`.
fn contains(start: u64, len: u64, x: u64) -> bool {
    start <= x && x <= last_byte(start, len)
}

/// Whether two ranges share at least one byte.
fn overlap(start1: u64, len1: u64, start2: u64, len2: u64) -> bool {
    start1.max(start2) <= last_byte(start1, len1).min(last_byte(start2, len2))
}

/// Whether two ranges overlap or are adjacent, i.e. their union is one range.
fn touch(start1: u64, len1: u64, start2: u64, len2: u64) -> bool {
    let first_gap_byte = last_byte(start1, len1).min(last_byte(start2, len2)) as u128 + 1;
    start1.max(start2) as u128 <= first_gap_byte
}

/// Picks one of [`OWNERS`].
fn any_owner() -> Owner {
    if kani::any() {
        OWNERS[0]
    } else {
        OWNERS[1]
    }
}

/// A lock with an owner from [`OWNERS`] and an arbitrary mode and range.
fn any_lock() -> ActiveLock {
    let (caller_name, system_identifier) = any_owner();
    ActiveLock::new(
        caller_name.to_string(),
        system_identifier,
        kani::any(),
        kani::any(),
        kani::any(),
        OpaqueHandle::new(Vec::new()).unwrap(),
    )
    .unwrap()
}

/// `N` arbitrary locks, see [`any_lock`].
fn any_locks<const N: usize>() -> Vec<ActiveLock> {
    (0..N).map(|_| any_lock()).collect()
}

/// Whether `lock` belongs to `owner`.
fn is_owned_by(lock: &ActiveLock, owner: Owner) -> bool {
    lock.caller_name == owner.0 && lock.system_identifier == owner.1
}

/// Whether `owner` holds byte `x` in mode `exclusive` according to `locks`.
fn holds(locks: &[ActiveLock], owner: Owner, exclusive: bool, x: u64) -> bool {
    locks.iter().any(|lock| {
        is_owned_by(lock, owner)
            && lock.exclusive == exclusive
            && contains(lock.offset, lock.length, x)
    })
}
