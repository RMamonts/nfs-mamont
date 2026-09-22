use crate::service::nlm::lock_types::ActiveLock;
use std::io::Error;

/// Length value that means "lock until end-of-file".
/// The lock covers all bytes from `offset` to EOF.
const LEN_REMAINING: u64 = 0;

/// Returns `true` when the two byte-range intervals `[start, start+len)` overlap.
/// A length of [`LEN_REMAINING`] is interpreted as "to end-of-file" (i.e. `u64::MAX`).
pub fn ranges_overlap(start1: u64, len1: u64, start2: u64, len2: u64) -> bool {
    let end1 = calculate_end_of_interval(start1, len1);
    let end2 = calculate_end_of_interval(start2, len2);
    start1 <= end2 && start2 <= end1
}

/// A length of [`LEN_REMAINING`] is interpreted as "to end-of-file" (i.e. `u64::MAX`).
pub fn calculate_end_of_interval(start: u64, len: u64) -> u64 {
    match len {
        LEN_REMAINING => u64::MAX,
        _ => start.saturating_add(len).saturating_sub(1),
    }
}

/// Returns the fragments of `lock` that remain after removing the byte range
/// `[unlock_start, unlock_end]`.
///
/// Returns an empty [`Vec`] when the unlock range fully covers the lock.
/// Returns one element when the unlock trims the lock from one side only,
/// and two elements when the unlock splits the lock in the middle.
pub fn split_lock(
    lock: ActiveLock,
    unlock_start: u64,
    unlock_len: u64,
) -> Result<Vec<ActiveLock>, Error> {
    let lock_start = lock.offset;
    let lock_len = lock.length;
    let lock_end = calculate_end_of_interval(lock_start, lock_len);
    let unlock_end = calculate_end_of_interval(unlock_start, unlock_len);

    let mut fragments = Vec::new();

    if lock_start < unlock_start {
        fragments.push(ActiveLock::new(
            lock.caller_name.clone(),
            lock.system_identifier,
            lock.exclusive,
            lock_start,
            unlock_start - lock_start,
            lock.opaque_handle.clone(),
        )?);
    }

    if unlock_end < lock_end {
        let right_len = if lock_len == 0 { 0 } else { lock_end - unlock_end };
        fragments.push(ActiveLock::new(
            lock.caller_name,
            lock.system_identifier,
            lock.exclusive,
            unlock_end + 1,
            right_len,
            lock.opaque_handle,
        )?);
    }

    Ok(fragments)
}

/// Removes locks owned by `(caller_name, system_identifier)` that overlap with
/// `[start, start+len)` from `locks`, keeping the non-overlapping parts via [`split_lock`].
pub fn drain_overlapping(
    locks: &mut Vec<ActiveLock>,
    caller_name: &str,
    system_identifier: i32,
    start: u64,
    len: u64,
) -> Result<(), Error> {
    let mut i = 0;
    while i < locks.len() {
        if locks[i].caller_name != caller_name || locks[i].system_identifier != system_identifier {
            i += 1;
            continue;
        }
        if !ranges_overlap(locks[i].offset, locks[i].length, start, len) {
            i += 1;
            continue;
        }
        let old = locks.swap_remove(i);
        locks.extend(split_lock(old, start, len)?);
    }
    Ok(())
}

/// Merges adjacent or overlapping lock ranges from the same owner
/// `(caller_name, system_identifier, exclusive)`.
///
/// For example, `[0,5)` and `[5,10)` become `[0,10)`,
/// and `[0,5)` with `[3,10)` also becomes `[0,10)`.
pub fn merge_adjacent(locks: &mut Vec<ActiveLock>) {
    locks.sort_by(|a, b| {
        a.caller_name
            .cmp(&b.caller_name)
            .then(a.system_identifier.cmp(&b.system_identifier))
            .then(a.exclusive.cmp(&b.exclusive))
            .then(a.offset.cmp(&b.offset))
    });

    let mut write = 0;
    for read in 1..locks.len() {
        if locks[write].caller_name == locks[read].caller_name
            && locks[write].system_identifier == locks[read].system_identifier
            && locks[write].exclusive == locks[read].exclusive
        {
            let write_end = calculate_end_of_interval(locks[write].offset, locks[write].length);
            let read_end = calculate_end_of_interval(locks[read].offset, locks[read].length);

            let adjacent_or_overlapping =
                if write_end == u64::MAX { true } else { write_end + 1 >= locks[read].offset };

            if adjacent_or_overlapping {
                if locks[write].length == 0 || locks[read].length == 0 {
                    locks[write].length = 0;
                } else {
                    let new_end = std::cmp::max(write_end, read_end);
                    locks[write].length = new_end - locks[write].offset + 1;
                }
                continue;
            }
        }
        write += 1;
        if write != read {
            locks.swap(write, read);
        }
    }
    locks.truncate(write + 1);
}
