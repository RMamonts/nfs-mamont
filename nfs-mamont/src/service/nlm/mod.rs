//! Server-side state and handlers for the NLM v4 RPC program.
//!
//! This module provides an in-memory lock manager that tracks active locks
//! grouped by file handle. The registry supports shared/exclusive semantics
//! and range-based conflict detection.
//!
//! The service implements `Lock`, `Unlock`, `Test` and `Cancel`
//! procedure traits from `crate::nlm::procedures`.

use std::collections::HashMap;

use crate::consts::nfsv3::NFS3_FHSIZE;
use crate::nlm::cookie::Cookie;
use crate::nlm::holder::Nlm4Holder;
use crate::nlm::OpaqueHandle;

mod cancel;
mod lock;
mod test;
mod unlock;

/// A held lock with full owner identity and state.
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
    /// TODO: Implement asynchronous callbacks
    #[allow(dead_code)]
    cookie: Cookie,
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
    by_file: HashMap<[u8; NFS3_FHSIZE], Vec<ActiveLock>>,
    /// Blocked lock requests awaiting grant.
    pending: HashMap<[u8; NFS3_FHSIZE], Vec<PendingLock>>,
}

impl LockRegistry {
    /// Creates an empty lock registry with no active or pending locks.
    fn new() -> LockRegistry {
        LockRegistry { by_file: HashMap::new(), pending: HashMap::new() }
    }

    /// Looks for an existing lock that would conflict with a new lock request.
    fn find_conflict(
        &self,
        file_handle: &[u8; NFS3_FHSIZE],
        request_exclusive: bool,
        request_offset: u64,
        request_length: u64,
    ) -> Option<Nlm4Holder> {
        let locks = self.by_file.get(file_handle)?;
        for lock in locks {
            if !request_exclusive && !lock.exclusive {
                continue;
            }
            if !ranges_overlap(lock.offset, lock.length, request_offset, request_length) {
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
    fn remove_pending(&mut self, file_handle: &[u8; NFS3_FHSIZE], target: &PendingLock) -> bool {
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

    /// Removes `target` from the active-lock list for `file_handle`.
    /// Matching uses `PartialEq` (caller_name, system_identifier, offset, length).
    fn remove_by_owner(&mut self, file_handle: &[u8; NFS3_FHSIZE], target: &ActiveLock) {
        let active_locks = match self.by_file.get_mut(file_handle) {
            Some(locks) => locks,
            None => return,
        };

        active_locks.retain(|lock| *lock != *target);

        if active_locks.is_empty() {
            self.by_file.remove(file_handle);
        }
    }

    /// Promotes pending lock requests that no longer conflict with active locks.
    ///
    /// Called after releasing an active lock ([`remove_by_owner`]) to check
    /// whether any previously blocked request can now be granted.
    ///
    /// Each non-conflicting request is moved into [`by_file`] as an [`ActiveLock`]
    /// and included in the returned vector. Requests that still conflict are
    /// kept in the pending queue.
    ///
    /// ### Parameters
    /// * `file_handle` — file whose pending queue should be rechecked.
    ///
    /// ### Returns
    /// A vector of [`PendingLock`]s that have been granted —
    /// the caller should send `NLMPROC4_GRANTED` for each one.
    fn grant_pending(&mut self, file_handle: &[u8; NFS3_FHSIZE]) -> Vec<PendingLock> {
        let pending_requests = self.pending.remove(file_handle).unwrap_or_default();

        let (granted, still_pending): (Vec<PendingLock>, Vec<PendingLock>) =
            pending_requests.into_iter().partition(|request| {
                self.find_conflict(file_handle, request.exclusive, request.offset, request.length)
                    .is_none()
            });

        for request in &granted {
            self.by_file.entry(*file_handle).or_default().push(ActiveLock {
                caller_name: request.caller_name.clone(),
                system_identifier: request.system_identifier,
                exclusive: request.exclusive,
                offset: request.offset,
                length: request.length,
                opaque_handle: request.opaque_handle.clone(),
            });
        }

        if !still_pending.is_empty() {
            self.pending.insert(*file_handle, still_pending);
        }

        granted
    }
}

/// Length value that means "lock until end-of-file".
/// The lock covers all bytes from `offset` to EOF.
const LEN_REMAINING: u64 = 0;

/// Returns `true` when the two byte-range intervals `[start, start+len)` overlap.
/// A length of [`LEN_REMAINING`] is interpreted as "to end-of-file" (i.e. `u64::MAX`).
fn ranges_overlap(start1: u64, len1: u64, start2: u64, len2: u64) -> bool {
    let end1 = calculate_end_of_interval(start1, len1);
    let end2 = calculate_end_of_interval(start2, len2);
    start1 <= end2 && start2 <= end1
}

/// A length of [`LEN_REMAINING`] is interpreted as "to end-of-file" (i.e. `u64::MAX`).
fn calculate_end_of_interval(start: u64, len: u64) -> u64 {
    match len {
        LEN_REMAINING => u64::MAX,
        _ => start.saturating_add(len).saturating_sub(1),
    }
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

#[cfg(test)]
mod tests;
