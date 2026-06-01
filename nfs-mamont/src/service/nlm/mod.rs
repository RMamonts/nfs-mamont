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
use crate::nlm::holder::Nlm4Holder;
use crate::nlm::OpaqueHandle;
use crate::service::mount::ExportEntryWrapper;

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

/// In-memory collection of all active locks grouped by file handle.
#[derive(Default)]
struct LockRegistry {
    /// Locks indexed by file handle for fast conflict checks.
    by_file: HashMap<[u8; NFS3_FHSIZE], Vec<ActiveLock>>,
}

impl LockRegistry {
    /// Looks for an existing lock that would conflict with a new lock
    /// request described by the given parameters.
    fn find_conflict(
        &self,
        file_handle: &[u8; NFS3_FHSIZE],
        exclusive: bool,
        offset: u64,
        length: u64,
    ) -> Option<Nlm4Holder> {
        let locks = self.by_file.get(file_handle)?;
        for lock in locks {
            if !exclusive && !lock.exclusive {
                continue;
            }
            if ranges_overlap(lock.offset, lock.length, offset, length) {
                return Some(Nlm4Holder::new(
                    lock.exclusive,
                    lock.system_identifier,
                    lock.opaque_handle.clone(),
                    lock.offset,
                    lock.length,
                ));
            }
        }
        None
    }

    /// Removes a lock matching `(file_handle, caller_name, system_identifier, offset, length)`.
    fn remove_by_owner(
        &mut self,
        file_handle: &[u8; NFS3_FHSIZE],
        caller_name: &str,
        system_identifier: i32,
        offset: u64,
        length: u64,
    ) {
        let Some(locks) = self.by_file.get_mut(file_handle) else { return };
        locks.retain(|l| {
            l.caller_name != caller_name
                || l.system_identifier != system_identifier
                || l.offset != offset
                || l.length != length
        });
        if locks.is_empty() {
            self.by_file.remove(file_handle);
        }
    }
}

/// Returns `true` when the two byte-range intervals `[start, start+len)`
/// overlap. A length of `0` is interpreted as "to end-of-file" (i.e.
/// `u64::MAX`).
fn ranges_overlap(start1: u64, len1: u64, start2: u64, len2: u64) -> bool {
    let end1 = if len1 == 0 { u64::MAX } else { start1.saturating_add(len1).saturating_sub(1) };
    let end2 = if len2 == 0 { u64::MAX } else { start2.saturating_add(len2).saturating_sub(1) };
    start1 <= end2 && start2 <= end1
}

/// In-memory state backing the NLM v4 service implementation.
///
/// Holds a lock registry protected by a read-write lock so that
/// multiple `TEST` (read-only) requests can proceed concurrently
/// while `LOCK`/`UNLOCK`/`CANCEL` (write) requests are serialised.
#[derive(Default)]
pub struct NlmService {
    /// Active locks grouped by file handle.
    locks: tokio::sync::RwLock<LockRegistry>,
}

impl NlmService {
    /// Creates an empty [`NlmService`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new [`NlmService`] with the given export entries.
    ///
    /// The export list is accepted for API compatibility with the MOUNT
    /// service but is not currently used for access control.
    pub fn with_exports(_entries: Vec<ExportEntryWrapper>) -> Self {
        Self::default()
    }
}

#[cfg(test)]
pub(crate) fn handle(byte: u8) -> [u8; NFS3_FHSIZE] {
    [byte; NFS3_FHSIZE]
}

#[cfg(test)]
pub(crate) fn opaque(val: u8) -> OpaqueHandle {
    OpaqueHandle::new([val; crate::consts::nlm::OPAQUE_HANDLE_SIZE])
}

#[cfg(test)]
use crate::nlm::cookie::Cookie;
#[cfg(test)]
use crate::nlm::lock::Nlm4Lock;
#[cfg(test)]
use crate::nlm::procedures::lock::Nlm4LockArgs;
#[cfg(test)]
use crate::vfs::file::Handle;

#[cfg(test)]
fn push_lock(reg: &mut LockRegistry, fh_byte: u8, exclusive: bool, offset: u64, length: u64) {
    reg.by_file.entry(handle(fh_byte)).or_default().push(ActiveLock {
        caller_name: "a".into(),
        system_identifier: 1,
        exclusive,
        offset,
        length,
        opaque_handle: opaque(1),
    });
}

#[cfg(test)]
pub(crate) fn lock_args(
    fh_byte: u8,
    exclusive: bool,
    offset: u64,
    length: u64,
    caller: &str,
    pid: i32,
) -> Nlm4LockArgs {
    Nlm4LockArgs {
        cookie: Cookie::new(0),
        block: false,
        exclusive,
        lock: Nlm4Lock {
            caller_name: caller.into(),
            file_handle: Handle(handle(fh_byte)),
            opaque_handle: opaque(1),
            system_identifier: pid,
            lock_offset: offset,
            lock_length: length,
        },
        reclaim: false,
        state: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nlm::cookie::Cookie;
    use crate::nlm::lock::Nlm4Lock;
    use crate::nlm::procedures::lock::{Lock, Nlm4LockArgs};
    use crate::nlm::procedures::unlock::{Nlm4UnlockArgs, Unlock};
    use crate::nlm::Nlm4Stats;
    use crate::vfs::file::Handle;

    // --- ranges_overlap ---

    #[test]
    fn overlapping_ranges_detect_overlap() {
        assert!(ranges_overlap(0, 10, 5, 10));
    }

    #[test]
    fn non_overlapping_ranges_no_overlap() {
        assert!(!ranges_overlap(0, 10, 10, 10));
    }

    #[test]
    fn identical_ranges_overlap() {
        assert!(ranges_overlap(42, 100, 42, 100));
    }

    #[test]
    fn inner_range_contained_overlaps() {
        assert!(ranges_overlap(0, 100, 25, 50));
    }

    #[test]
    fn zero_length_ranges_overlap() {
        assert!(ranges_overlap(0, 0, 0, 0));
    }

    #[test]
    fn zero_length_means_to_eof() {
        assert!(ranges_overlap(0, 0, 100, 50));
    }

    #[test]
    fn zero_length_does_not_overlap_before() {
        assert!(!ranges_overlap(100, 0, 0, 50));
    }

    // --- LockRegistry::find_conflict ---

    #[test]
    fn no_conflict_when_no_locks() {
        assert!(LockRegistry::default().find_conflict(&handle(1), true, 0, 100).is_none());
    }

    #[test]
    fn exclusive_conflicts_with_existing_exclusive() {
        let mut reg = LockRegistry::default();
        push_lock(&mut reg, 1, true, 0, 100);
        assert!(reg.find_conflict(&handle(1), true, 10, 20).is_some());
    }

    #[test]
    fn shared_does_not_conflict_with_shared() {
        let mut reg = LockRegistry::default();
        push_lock(&mut reg, 1, false, 0, 100);
        assert!(reg.find_conflict(&handle(1), false, 10, 20).is_none());
    }

    #[test]
    fn shared_conflicts_with_exclusive() {
        let mut reg = LockRegistry::default();
        push_lock(&mut reg, 1, true, 0, 100);
        assert!(reg.find_conflict(&handle(1), false, 10, 20).is_some());
    }

    #[test]
    fn no_conflict_on_different_file() {
        let mut reg = LockRegistry::default();
        push_lock(&mut reg, 1, true, 0, 100);
        assert!(reg.find_conflict(&handle(2), true, 0, 100).is_none());
    }

    #[test]
    fn no_conflict_when_ranges_dont_overlap() {
        let mut reg = LockRegistry::default();
        push_lock(&mut reg, 1, true, 0, 10);
        assert!(reg.find_conflict(&handle(1), true, 10, 10).is_none());
    }

    #[test]
    fn find_conflict_returns_holder_with_correct_fields() {
        let mut reg = LockRegistry::default();
        reg.by_file.entry(handle(1)).or_default().push(ActiveLock {
            caller_name: "a".into(),
            system_identifier: 42,
            exclusive: true,
            offset: 10,
            length: 20,
            opaque_handle: opaque(7),
        });
        let holder = reg.find_conflict(&handle(1), true, 0, 100).unwrap();
        assert!(holder.exclusive);
        assert_eq!(holder.system_identifier, 42);
        assert_eq!(holder.opaque_handle.as_bytes(), &[7; crate::consts::nlm::OPAQUE_HANDLE_SIZE]);
        assert_eq!(holder.lock_offset, 10);
        assert_eq!(holder.lock_length, 20);
    }

    // --- LockRegistry::remove_by_owner ---

    #[test]
    fn remove_by_owner_removes_matching_lock() {
        let mut reg = LockRegistry::default();
        reg.by_file.entry(handle(1)).or_default().push(ActiveLock {
            caller_name: "alice".into(),
            system_identifier: 100,
            exclusive: true,
            offset: 0,
            length: 50,
            opaque_handle: opaque(1),
        });
        reg.remove_by_owner(&handle(1), "alice", 100, 0, 50);
        assert!(reg.by_file.is_empty());
    }

    #[test]
    fn remove_by_owner_removes_only_different_owner() {
        let mut reg = LockRegistry::default();
        let locks = reg.by_file.entry(handle(1)).or_default();
        locks.push(ActiveLock {
            caller_name: "alice".into(),
            system_identifier: 100,
            exclusive: true,
            offset: 0,
            length: 50,
            opaque_handle: opaque(1),
        });
        locks.push(ActiveLock {
            caller_name: "bob".into(),
            system_identifier: 200,
            exclusive: true,
            offset: 60,
            length: 50,
            opaque_handle: opaque(2),
        });
        reg.remove_by_owner(&handle(1), "alice", 100, 0, 50);
        assert_eq!(reg.by_file.get(&handle(1)).unwrap().len(), 1);
        assert_eq!(reg.by_file.get(&handle(1)).unwrap()[0].caller_name, "bob");
    }

    #[test]
    fn remove_by_owner_removes_only_matching_range() {
        let mut reg = LockRegistry::default();
        let locks = reg.by_file.entry(handle(1)).or_default();
        locks.push(ActiveLock {
            caller_name: "Alice".into(),
            system_identifier: 100,
            exclusive: true,
            offset: 0,
            length: 50,
            opaque_handle: opaque(1),
        });
        locks.push(ActiveLock {
            caller_name: "Alice".into(),
            system_identifier: 100,
            exclusive: true,
            offset: 100,
            length: 50,
            opaque_handle: opaque(2),
        });
        reg.remove_by_owner(&handle(1), "Alice", 100, 0, 50);
        assert_eq!(reg.by_file.get(&handle(1)).unwrap().len(), 1);
        assert_eq!(reg.by_file.get(&handle(1)).unwrap()[0].offset, 100);
    }

    #[test]
    fn remove_by_owner_noop_on_nonexistent_file() {
        LockRegistry::default().remove_by_owner(&handle(99), "nobody", 0, 0, 0);
    }

    #[test]
    fn remove_by_owner_cleans_up_empty_vec() {
        let mut reg = LockRegistry::default();
        push_lock(&mut reg, 1, true, 0, 10);
        reg.remove_by_owner(&handle(1), "a", 1, 0, 10);
        assert!(!reg.by_file.contains_key(&handle(1)));
    }

    #[test]
    fn remove_by_owner_noop_when_range_differs() {
        let mut reg = LockRegistry::default();
        push_lock(&mut reg, 1, true, 0, 50);
        reg.remove_by_owner(&handle(1), "a", 1, 100, 50);
        assert!(reg.by_file.contains_key(&handle(1)));
        assert_eq!(reg.by_file.get(&handle(1)).unwrap().len(), 1);
    }

    // --- NlmService constructor ---

    #[test]
    fn with_exports_creates_empty_service() {
        let svc = NlmService::with_exports(vec![]);
        assert!(svc.locks.try_read().is_ok(), "service should not block on empty read");
    }

    // --- Integration tests spanning multiple procedures ---

    #[tokio::test]
    async fn lock_unlock_lock_sequence_same_client() {
        let svc = NlmService::default();
        let args = Nlm4LockArgs {
            cookie: Cookie::new(1),
            block: false,
            exclusive: true,
            lock: Nlm4Lock {
                caller_name: "client1".into(),
                file_handle: Handle(handle(1)),
                opaque_handle: opaque(1),
                system_identifier: 100,
                lock_offset: 0,
                lock_length: 100,
            },
            reclaim: false,
            state: 0,
        };
        assert_eq!(svc.lock(args).await.stat, Nlm4Stats::Granted);

        let unlock_args = Nlm4UnlockArgs {
            cookie: Cookie::new(2),
            lock: Nlm4Lock {
                caller_name: "client1".into(),
                file_handle: Handle(handle(1)),
                opaque_handle: opaque(1),
                system_identifier: 100,
                lock_offset: 0,
                lock_length: 100,
            },
        };
        assert_eq!(svc.unlock(unlock_args).await.stat, Nlm4Stats::Granted);

        let relock = Nlm4LockArgs {
            cookie: Cookie::new(3),
            block: false,
            exclusive: true,
            lock: Nlm4Lock {
                caller_name: "client1".into(),
                file_handle: Handle(handle(1)),
                opaque_handle: opaque(1),
                system_identifier: 100,
                lock_offset: 0,
                lock_length: 100,
            },
            reclaim: false,
            state: 0,
        };
        assert_eq!(svc.lock(relock).await.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn multiple_clients_lock_different_ranges_on_same_file() {
        let svc = NlmService::default();
        let args1 = Nlm4LockArgs {
            cookie: Cookie::new(1),
            block: false,
            exclusive: true,
            lock: Nlm4Lock {
                caller_name: "client1".into(),
                file_handle: Handle(handle(1)),
                opaque_handle: opaque(1),
                system_identifier: 100,
                lock_offset: 0,
                lock_length: 50,
            },
            reclaim: false,
            state: 0,
        };
        assert_eq!(svc.lock(args1).await.stat, Nlm4Stats::Granted);

        let args2 = Nlm4LockArgs {
            cookie: Cookie::new(2),
            block: false,
            exclusive: true,
            lock: Nlm4Lock {
                caller_name: "client2".into(),
                file_handle: Handle(handle(1)),
                opaque_handle: opaque(2),
                system_identifier: 200,
                lock_offset: 60,
                lock_length: 50,
            },
            reclaim: false,
            state: 0,
        };
        assert_eq!(svc.lock(args2).await.stat, Nlm4Stats::Granted);
    }
}
