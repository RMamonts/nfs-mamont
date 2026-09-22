use crate::nlm::holder::Nlm4Holder;
use crate::service::nlm::arithmetic::drain_overlapping;
use crate::service::nlm::arithmetic::merge_adjacent;
use crate::service::nlm::arithmetic::ranges_overlap;
use crate::service::nlm::lock_types::ActiveLock;
use crate::service::nlm::lock_types::PendingLock;
use crate::vfs::file::Handle;
use std::collections::HashMap;
use std::io::Error;

/// In-memory collection of all active locks grouped by file handle.
pub struct LockRegistry {
    /// Locks indexed by file handle for fast conflict checks.
    pub by_file: HashMap<Handle, Vec<ActiveLock>>,
    /// Blocked lock requests awaiting grant.
    pub pending: HashMap<Handle, Vec<PendingLock>>,
}

impl LockRegistry {
    /// Creates an empty lock registry with no active or pending locks.
    pub fn new() -> LockRegistry {
        LockRegistry { by_file: HashMap::new(), pending: HashMap::new() }
    }

    /// Looks for an existing lock that would conflict with `request`.
    /// Locks owned by the same `(caller_name, system_identifier, opaque_handle)`
    /// are skipped — a client re-requesting its own range is not a conflict.
    /// Returns `true` if there is an active lock from the same `(caller_name, system_identifier)`
    /// with the same `exclusive` mode and an overlapping range.
    pub fn has_active_lock(&self, file_handle: &Handle, request: &ActiveLock) -> bool {
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
    pub fn find_conflict(&self, file_handle: &Handle, request: &ActiveLock) -> Option<Nlm4Holder> {
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
    pub fn remove_pending(&mut self, file_handle: &Handle, target: &PendingLock) -> bool {
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
    pub fn remove_by_owner(
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
    pub fn push_or_replace(
        &mut self,
        file_handle: Handle,
        new_lock: ActiveLock,
    ) -> Result<(), Error> {
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
    pub fn grant_pending(&mut self, file_handle: &Handle) -> Result<Vec<PendingLock>, Error> {
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
