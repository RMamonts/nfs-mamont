//! Server-side state and handlers for the NLM v4 RPC program.
//!
//! This module provides an in-memory lock manager that tracks active locks
//! grouped by file handle. The registry supports shared/exclusive semantics
//! and range-based conflict detection.
//!
//! The service implements `Lock`, `Unlock`, `Test` and `Cancel`
//! procedure traits from `crate::nlm::procedures`.

use std::collections::HashMap;
use std::io::Error;

use crate::consts::nlm;
use crate::nlm::cookie::Cookie;
use crate::nlm::holder::Nlm4Holder;
use crate::nlm::OpaqueHandle;
use crate::vfs::file::Handle;

mod cancel;
mod lock;
mod test;
mod unlock;

#[cfg(test)]
mod tests;

/// A held lock with full owner identity and state.
#[derive(Clone)]
struct ActiveLock {
    /// Name of the client host that owns the lock.
    caller_name: String,
    /// PID of the process on the client that owns the lock.
    system_identifier: i32,
    /// `true` for exclusive lock, `false` for shared lock.
    exclusive: bool,
    /// Starting offset of the locked region (in bytes).
    offset: u64,
    /// Length of the locked region. A value of `0` means "to end-of-file".
    length: u64,
    /// Opaque handle identifying the lock owner (returned in TEST responses).
    opaque_handle: OpaqueHandle,
}

/// # Errors
///
/// Returns [`Error`] if:
/// - `caller_name` is empty.
/// - `caller_name` is longer than [`LM_MAXSTRLEN`](nlm::LM_MAXSTRLEN).
fn check_caller_name(caller_name: &str) -> Result<(), Error> {
    if caller_name.is_empty() {
        return Err(Error::new(std::io::ErrorKind::InvalidInput, "caller_name must not be empty"));
    }

    if caller_name.len() > nlm::LM_MAXSTRLEN {
        return Err(Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("caller_name is too long (max {})", nlm::LM_MAXSTRLEN),
        ));
    }
    Ok(())
}

impl ActiveLock {
    /// Creates a new [`ActiveLock`] with validation.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if:
    /// - `caller_name` is empty.
    /// - `caller_name` is longer than [`LM_MAXSTRLEN`](nlm::LM_MAXSTRLEN).
    pub fn new(
        caller_name: String,
        system_identifier: i32,
        exclusive: bool,
        offset: u64,
        length: u64,
        opaque_handle: OpaqueHandle,
    ) -> Result<Self, Error> {
        check_caller_name(&caller_name)?;

        Ok(ActiveLock { caller_name, system_identifier, exclusive, offset, length, opaque_handle })
    }
}

/// Equality compares only the unlock-identity fields
/// (`caller_name`, `system_identifier`, `offset`, `length`).
/// `exclusive` and `opaque_handle` are intentionally ignored —
/// UNLOCK identifies a lock by owner + range, not by mode or handle.
impl PartialEq for ActiveLock {
    fn eq(&self, other: &Self) -> bool {
        self.caller_name == other.caller_name
            && self.system_identifier == other.system_identifier
            && self.offset == other.offset
            && self.length == other.length
    }
}

/// A blocked (pending) lock request waiting to be granted.
struct PendingLock {
    /// Name of the client host that owns the lock.
    caller_name: String,
    /// PID of the process on the client that owns the lock.
    system_identifier: i32,
    /// `true` for exclusive lock, `false` for shared lock.
    exclusive: bool,
    /// Starting byte offset of the requested lock region.
    offset: u64,
    /// Length of the requested lock region. `0` means to end-of-file.
    length: u64,
    /// Opaque handle identifying the lock owner (used in GRANTED callback).
    opaque_handle: OpaqueHandle,
    /// The cookie from the original blocking LOCK request.
    /// TODO: Needed for NLMPROC4_GRANTED callback (#267).
    #[allow(dead_code)]
    cookie: Cookie,
}

impl PendingLock {
    /// Creates a new [`PendingLock`] with validation.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if:
    /// - `caller_name` is empty.
    /// - `caller_name` is longer than [`LM_MAXSTRLEN`](nlm::LM_MAXSTRLEN).
    pub fn new(
        caller_name: String,
        system_identifier: i32,
        exclusive: bool,
        offset: u64,
        length: u64,
        opaque_handle: OpaqueHandle,
        cookie: Cookie,
    ) -> Result<Self, Error> {
        check_caller_name(&caller_name)?;
        Ok(PendingLock {
            caller_name,
            system_identifier,
            exclusive,
            offset,
            length,
            opaque_handle,
            cookie,
        })
    }
}

/// Converts a [`PendingLock`] reference into an [`ActiveLock`] by copying all shared fields.
/// The `cookie` field from the pending request is intentionally dropped,
/// as it is only relevant for the GRANTED callback and has no meaning for an active lock.
impl From<&PendingLock> for ActiveLock {
    fn from(p: &PendingLock) -> Self {
        ActiveLock::new(
            p.caller_name.clone(),
            p.system_identifier,
            p.exclusive,
            p.offset,
            p.length,
            p.opaque_handle.clone(),
        )
        .expect("PendingLock must have valid caller_name")
    }
}

/// Equality compares all identity fields needed to match a `CANCEL` request.
/// `cookie` is excluded because it is a request-scoped transient identifier,
/// not an attribute of the lock itself.
impl PartialEq for PendingLock {
    fn eq(&self, other: &Self) -> bool {
        self.caller_name == other.caller_name
            && self.system_identifier == other.system_identifier
            && self.exclusive == other.exclusive
            && self.offset == other.offset
            && self.length == other.length
            && self.opaque_handle == other.opaque_handle
    }
}

/// In-memory collection of all active locks grouped by file handle.
struct LockRegistry {
    /// Locks indexed by file handle for fast conflict checks.
    by_file: HashMap<Handle, Vec<ActiveLock>>,
    /// Blocked lock requests awaiting grant.
    pending: HashMap<Handle, Vec<PendingLock>>,
}

impl LockRegistry {
    /// Creates an empty lock registry with no active or pending locks.
    fn new() -> LockRegistry {
        LockRegistry { by_file: HashMap::new(), pending: HashMap::new() }
    }

    /// Looks for an existing lock that would conflict with `request`.
    /// Locks owned by the same `(caller_name, system_identifier, opaque_handle)`
    /// are skipped — a client re-requesting its own range is not a conflict.
    /// Returns `true` if there is an active lock from the same `(caller_name, system_identifier)`
    /// with the same `exclusive` mode and an overlapping range.
    fn has_active_lock(&self, file_handle: &Handle, request: &ActiveLock) -> bool {
        self.by_file.get(file_handle).is_some_and(|locks| {
            locks.iter().any(|lock| {
                lock.caller_name == request.caller_name
                    && lock.system_identifier == request.system_identifier
                    && lock.exclusive == request.exclusive
                    && ranges_overlap(lock.offset, lock.length, request.offset, request.length)
            })
        })
    }

    /// Looks for an existing lock that would conflict with `request`.
    /// Locks owned by the same `(caller_name, system_identifier, opaque_handle)`
    /// are skipped — a client re-requesting its own range is not a conflict.
    fn find_conflict(&self, file_handle: &Handle, request: &ActiveLock) -> Option<Nlm4Holder> {
        let locks = self.by_file.get(file_handle)?;
        for lock in locks {
            let is_same_owner = lock.caller_name == request.caller_name
                && lock.system_identifier == request.system_identifier
                && lock.opaque_handle == request.opaque_handle;

            if is_same_owner {
                continue;
            }
            if !request.exclusive && !lock.exclusive {
                continue;
            }
            if !ranges_overlap(lock.offset, lock.length, request.offset, request.length) {
                continue;
            }
            return Some(Nlm4Holder::new(
                lock.exclusive,
                lock.system_identifier,
                lock.opaque_handle.clone(),
                lock.offset,
                lock.length,
            ));
        }
        None
    }

    /// Removes `target` from the pending queue for `file_handle`.
    /// Matching uses `PartialEq` (caller_name, system_identifier, exclusive,
    /// offset, length, opaque_handle — cookie is ignored).
    /// Returns `true` if a matching request was found and removed.
    fn remove_pending(&mut self, file_handle: &Handle, target: &PendingLock) -> bool {
        let pending_requests = match self.pending.get_mut(file_handle) {
            Some(requests) => requests,
            None => return false,
        };

        let number_of_ending_requests_before_retain = pending_requests.len();
        pending_requests.retain(|request| *request != *target);
        let has_request_been_deleted =
            pending_requests.len() < number_of_ending_requests_before_retain;

        if pending_requests.is_empty() {
            self.pending.remove(file_handle);
        }
        has_request_been_deleted
    }

    /// Removes active locks owned by `(caller_name, system_identifier)` that overlap
    /// with `[offset, offset+len)` from the active-lock list for `file_handle`.
    /// Non-overlapping portions are preserved via range splitting.
    fn remove_by_owner(
        &mut self,
        file_handle: &Handle,
        caller_name: &str,
        system_identifier: i32,
        offset: u64,
        len: u64,
    ) -> Result<(), Error> {
        let active_locks = match self.by_file.get_mut(file_handle) {
            Some(locks) => locks,
            None => return Ok(()),
        };

        drain_overlapping(active_locks, caller_name, system_identifier, offset, len)?;

        if active_locks.is_empty() {
            self.by_file.remove(file_handle);
        }
        Ok(())
    }

    /// Pushes a new active lock, replacing or trimming any existing same-owner
    /// locks that overlap with the range of `new_lock`.
    ///
    /// This prevents accumulation of duplicate or overlapping locks from the same client.
    /// A subsequent lock on the same range (e.g. upgrading from Shared to Exclusive)
    /// replaces the old entry instead of adding a second one.
    fn push_or_replace(&mut self, file_handle: Handle, new_lock: ActiveLock) -> Result<(), Error> {
        let locks = self.by_file.entry(file_handle).or_default();
        drain_overlapping(
            locks,
            &new_lock.caller_name,
            new_lock.system_identifier,
            new_lock.offset,
            new_lock.length,
        )?;
        locks.push(new_lock);
        merge_adjacent(locks);
        Ok(())
    }

    /// Promotes pending lock requests that no longer conflict with active locks.
    ///
    /// Called after releasing an active lock (`remove_by_owner`) to check
    /// whether any previously blocked request can now be granted.
    ///
    /// Each non-conflicting request is moved into `by_file` as an [`ActiveLock`]
    /// and included in the returned vector. Requests that still conflict are
    /// kept in the pending queue.
    ///
    /// ### Parameters
    /// * `file_handle` — file whose pending queue should be rechecked.
    ///
    /// ### Returns
    /// A vector of [`PendingLock`]s that have been granted —
    /// the caller should send `NLMPROC4_GRANTED` for each one.
    fn grant_pending(&mut self, file_handle: &Handle) -> Result<Vec<PendingLock>, Error> {
        let pending_requests = self.pending.remove(file_handle).unwrap_or_default();
        let mut granted: Vec<PendingLock> = Vec::new();
        let mut still_pending: Vec<PendingLock> = Vec::new();

        for request in pending_requests {
            let request_as_active: ActiveLock = (&request).into();
            if self.find_conflict(file_handle, &request_as_active).is_some() {
                still_pending.push(request);
            } else {
                self.push_or_replace((*file_handle).clone(), request_as_active)?;
                granted.push(request);
            }
        }

        if !still_pending.is_empty() {
            self.pending.insert((*file_handle).clone(), still_pending);
        }

        Ok(granted)
    }
}

/// Length value that means "lock until end-of-file".
/// The lock covers all bytes from `offset` to EOF.
const LEN_REMAINING: u64 = 0;

/// Returns `true` when the two byte-range intervals share at least one byte.
///
/// Ranges are compared as the *inclusive* intervals `[start, end]` that
/// [`calculate_end_of_interval`] builds, not as the half-open `[start, start + len)`
/// the wire format spells them in --- a lock of length 1 covers the byte at `start`
/// and conflicts with another lock on it. A length of [`LEN_REMAINING`] is
/// interpreted as "to end-of-file" (i.e. `u64::MAX`).
fn ranges_overlap(start1: u64, len1: u64, start2: u64, len2: u64) -> bool {
    let end1 = calculate_end_of_interval(start1, len1);
    let end2 = calculate_end_of_interval(start2, len2);
    start1 <= end2 && start2 <= end1
}

/// A length of [`LEN_REMAINING`] is interpreted as "to end-of-file" (i.e. `u64::MAX`).
///
/// The end is inclusive and is clamped at the last addressable byte for a range
/// that would run past the end of the 64-bit space. It is computed as
/// `start + (len - 1)` rather than `(start + len) - 1` so that the clamp can never
/// produce an end *before* `start`: with the latter, `start = u64::MAX` saturated
/// the sum and then subtracted one, yielding a backwards interval that overlapped
/// nothing --- not even itself.
fn calculate_end_of_interval(start: u64, len: u64) -> u64 {
    match len {
        LEN_REMAINING => u64::MAX,
        _ => start.saturating_add(len - 1),
    }
}

/// The inverse of [`calculate_end_of_interval`]: the length that covers the
/// inclusive range `[start, end]`.
///
/// A range ending at the last addressable byte has no representable length
/// (`end - start + 1` overflows), and that is precisely what [`LEN_REMAINING`]
/// encodes, so it is returned instead.
fn calculate_len_of_interval(start: u64, end: u64) -> u64 {
    // The inverse is only defined on a well-formed interval. Every caller gets one
    // from [`calculate_end_of_interval`], which is proved never to return an end
    // before its start, so this holds --- but the saturation below would quietly
    // turn a backwards interval into a length of 1 rather than say so.
    debug_assert!(end >= start, "interval end precedes its start");

    match end.saturating_sub(start).checked_add(1) {
        Some(len) => len,
        None => LEN_REMAINING,
    }
}

/// Returns the fragments of `lock` that remain after removing the byte range
/// `[unlock_start, unlock_end]`.
///
/// Returns an empty [`Vec`] when the unlock range fully covers the lock.
/// Returns one element when the unlock trims the lock from one side only,
/// and two elements when the unlock splits the lock in the middle.
///
/// Fragment lengths are computed here rather than through
/// [`calculate_len_of_interval`], which is what [`merge_adjacent`] uses: a fragment
/// of a lock that ran to end-of-file has to stay encoded as [`LEN_REMAINING`], and
/// the helper would hand back an explicit length instead. Both the client and
/// [`merge_adjacent`] read a length of `0` as "to end-of-file", so the distinction
/// is not cosmetic. The `if` guards below are what keep the arithmetic in range.
fn split_lock(
    lock: ActiveLock,
    unlock_start: u64,
    unlock_len: u64,
) -> Result<Vec<ActiveLock>, Error> {
    let lock_start = lock.offset;
    let lock_len = lock.length;
    let lock_end = calculate_end_of_interval(lock_start, lock_len);
    let unlock_end = calculate_end_of_interval(unlock_start, unlock_len);

    let mut fragments = Vec::new();

    // The left fragment is the inclusive range `[lock_start, unlock_start - 1]`, so
    // its length is the difference. The guard makes that difference at least 1, so
    // it can never come out as `LEN_REMAINING` by accident.
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

    // The right fragment is `[unlock_end + 1, lock_end]`, whose inclusive length is
    // `lock_end - unlock_end`. The guard gives `unlock_end < lock_end`, so the
    // subtraction never goes below 1 and `unlock_end + 1` cannot wrap.
    if unlock_end < lock_end {
        // A lock that ran to end-of-file keeps running there.
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

/// Removes locks owned by `(caller_name, system_identifier)` that overlap the
/// inclusive range `[start, calculate_end_of_interval(start, len)]` from `locks`,
/// keeping the non-overlapping parts via [`split_lock`].
fn drain_overlapping(
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
fn merge_adjacent(locks: &mut Vec<ActiveLock>) {
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
                    locks[write].length = calculate_len_of_interval(locks[write].offset, new_end);
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

/// In-memory state backing the NLM v4 service implementation.
///
/// Holds a lock registry protected by a read-write lock so that
/// multiple `TEST` (read-only) requests can proceed concurrently
/// while `LOCK`/`UNLOCK`/`CANCEL` (write) requests are serialised.
pub struct NlmService {
    /// Active locks grouped by file handle.
    locks: tokio::sync::RwLock<LockRegistry>,
}

impl Default for NlmService {
    /// Creates an empty [`NlmService`] with no locks registered.
    fn default() -> Self {
        NlmService { locks: tokio::sync::RwLock::new(LockRegistry::new()) }
    }
}

impl NlmService {
    /// Creates an empty [`NlmService`].
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(kani)]
mod verification {
    use super::*;

    /// The whole conflict detector rests on `[start, end]` being a well-formed
    /// inclusive interval. A backwards interval overlaps nothing --- not even
    /// itself --- so [`LockRegistry::find_conflict`] would hand two clients an
    /// exclusive lock over the same byte, and [`merge_adjacent`] would subtract
    /// with overflow. Both are reachable from the `offset`/`length` a client puts
    /// in an NLM `LOCK` request.
    #[kani::proof]
    fn end_of_interval_never_precedes_start() {
        let start: u64 = kani::any();
        let len: u64 = kani::any();

        assert!(calculate_end_of_interval(start, len) >= start);
    }

    /// [`calculate_len_of_interval`] inverts [`calculate_end_of_interval`], which
    /// is what makes [`merge_adjacent`] store back the range it just computed
    /// instead of a wrapped one.
    #[kani::proof]
    fn interval_length_round_trips() {
        let start: u64 = kani::any();
        let end: u64 = kani::any();
        kani::assume(end >= start);

        let len = calculate_len_of_interval(start, end);

        assert_eq!(calculate_end_of_interval(start, len), end);
    }

    /// A lock request always conflicts with an identical one already held.
    #[kani::proof]
    fn ranges_overlap_is_reflexive() {
        let start: u64 = kani::any();
        let len: u64 = kani::any();

        assert!(ranges_overlap(start, len, start, len));
    }

    /// [`LockRegistry::find_conflict`] compares the stored lock against the
    /// request in one order only, so the answer must not depend on that order.
    #[kani::proof]
    fn ranges_overlap_is_symmetric() {
        let start1: u64 = kani::any();
        let len1: u64 = kani::any();
        let start2: u64 = kani::any();
        let len2: u64 = kani::any();

        assert_eq!(
            ranges_overlap(start1, len1, start2, len2),
            ranges_overlap(start2, len2, start1, len1)
        );
    }

    /// [`ranges_overlap`] agrees with the set semantics it stands for: it is true
    /// exactly when some byte belongs to both ranges.
    #[kani::proof]
    fn ranges_overlap_matches_byte_membership() {
        let start1: u64 = kani::any();
        let len1: u64 = kani::any();
        let start2: u64 = kani::any();
        let len2: u64 = kani::any();

        let end1 = calculate_end_of_interval(start1, len1);
        let end2 = calculate_end_of_interval(start2, len2);

        if ranges_overlap(start1, len1, start2, len2) {
            // The later of the two starts is the first byte the ranges share.
            let witness = std::cmp::max(start1, start2);
            assert!(witness <= end1 && witness <= end2);
        } else {
            let byte: u64 = kani::any();
            assert!(!(byte >= start1 && byte <= end1 && byte >= start2 && byte <= end2));
        }
    }
}
