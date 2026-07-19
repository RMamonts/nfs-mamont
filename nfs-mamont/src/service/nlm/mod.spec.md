<!-- SPEC_HASH: 9bd5d2a2d117e6be85a2a83924763544cf89419a6bd4b5424c373f262e346b7a -->
# Module Specification

Module: nfs_mamont::service::nlm
Rust File: src/service/nlm/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`std::collections::HashMap`**:
    *   Used as the underlying storage mechanism for `LockRegistry`. It maps file handles to lists of active locks and pending requests, providing O(1) average access time to the lock state for a specific file.
*   **`std::io::Error`**:
    *   Used as the error type for validation functions (e.g., `check_caller_name`). It allows the module to signal invalid input (like empty or oversized strings) using standard Rust error handling.
*   **`crate::consts::nlm`**:
    *   Used to access the `LM_MAXSTRLEN` constant. This constant defines the maximum allowed length for the `caller_name` field in lock requests, ensuring compliance with the NLM protocol limits.
*   **`crate::nlm::cookie`**:
    *   Used to import the `Cookie` type. This type is stored within `PendingLock` to identify the specific transaction that resulted in the blocked request, which is necessary for the `NLMPROC4_GRANTED` callback.
*   **`crate::nlm::holder`**:
    *   Used to import the `Nlm4Holder` type. This type is returned by the `find_conflict` method to describe the owner of a conflicting lock to the client.
*   **`crate::nlm::OpaqueHandle`**:
    *   Used to store the opaque owner identifier within `ActiveLock` and `PendingLock`. This handle uniquely identifies the client or process owning the lock across the network.
*   **`crate::vfs::file`**:
    *   Used to import the `Handle` type. This type serves as the key in the `LockRegistry`'s internal maps, identifying the specific file on which locks are held or requested.
*   **`tokio::sync::RwLock`**:
    *   Used to wrap the `LockRegistry` inside `NlmService`. This provides asynchronous read-write locking, allowing concurrent read operations (like `TEST`) while serializing write operations (like `LOCK` and `UNLOCK`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Lock Registry State Management (`LockRegistry`)

**Intent:**
To maintain the authoritative in-memory state of all active and pending file locks, providing methods to query conflicts, add/remove locks, and manage the queue of blocked requests.

**Inputs:**
*   `file_handle`: The `Handle` identifying the target file.
*   `request`: An `ActiveLock` or `PendingLock` representing the operation to perform.

**Outputs:**
*   `Option<Nlm4Holder>`: Details of a conflicting lock, if found.
*   `Vec<PendingLock>`: A list of pending locks that have been granted during a `grant_pending` operation.
*   `bool`: Status indicating if a pending lock was successfully removed.

**Steps:**
1.  **Conflict Detection (`find_conflict`)**: Iterates through active locks for a given file. It skips locks owned by the same `(caller_name, system_identifier, opaque_handle)`. It checks for overlapping ranges and incompatible modes (exclusive vs. shared).
2.  **Lock Insertion (`push_or_replace`)**: Adds a new lock to the registry. It first calls `drain_overlapping` to remove or trim any existing locks owned by the same client that overlap with the new range. Finally, it calls `merge_adjacent` to consolidate adjacent locks.
3.  **Lock Removal (`remove_by_owner`)**: Removes locks matching the owner and range. It uses `drain_overlapping` to split existing locks if the unlock range only covers a portion of a held lock, preserving the remaining fragments.
4.  **Pending Queue Processing (`grant_pending`)**: After a lock is released, this method iterates through the pending queue for that file. It attempts to convert each `PendingLock` to an `ActiveLock` and checks for conflicts using `find_conflict`. Non-conflicting requests are moved to the active list and returned for notification.

**Edge Cases:**
*   **Zero Length**: A length of `0` is interpreted as "to end-of-file" (`u64::MAX`) in overlap calculations.
*   **Self-Conflict**: A client requesting a lock on a range they already own (same mode) is not considered a conflict.
*   **Empty Registry**: Operations on files with no locks or pending requests generally succeed without error (e.g., `remove_by_owner` returns `Ok(())`).

**Complexity:**
*   **Time**: O(N) for most operations where N is the number of locks on a specific file (due to linear scans for conflict detection, splitting, and merging).
*   **Space**: O(L) where L is the total number of active and pending locks across all files.

**Determinism:**
*   Deterministic.

### Mechanism 2: Range Arithmetic and Manipulation

**Intent:**
To implement the byte-range locking logic required by the NLM protocol, specifically handling partial unlocks (splitting) and lock consolidation (merging).

**Inputs:**
*   `lock`: The `ActiveLock` to manipulate.
*   `unlock_start`, `unlock_len`: The range to remove.
*   `locks`: A mutable list of locks to sort and merge.

**Outputs:**
*   `Vec<ActiveLock>`: Fragments of a lock after splitting.
*   Modified `locks` vector with merged entries.

**Steps:**
1.  **Overlap Calculation (`ranges_overlap`)**: Calculates the end of intervals (treating length 0 as EOF) and checks if intervals `[start1, end1]` and `[start2, end2]` intersect.
2.  **Lock Splitting (`split_lock`)**: Calculates the fragments of a lock that remain after removing a specific range. It can return 0, 1, or 2 fragments depending on whether the unlock range covers the whole lock, a side, or the middle.
3.  **Lock Merging (`merge_adjacent`)**: Sorts the lock list by owner and offset. It then iterates through the list, merging adjacent or overlapping locks that belong to the same owner and have the same exclusive mode.

**Edge Cases:**
*   **Splitting in Middle**: Unlocking bytes 5-10 of a lock holding 0-20 results in two fragments: 0-4 and 11-20.
*   **EOF Handling**: Merging logic correctly handles locks extending to EOF (length 0) by treating the end as `u64::MAX`.

**Complexity:**
*   **Time**: O(N log N) for `merge_adjacent` due to sorting; O(1) for `ranges_overlap`.
*   **Space**: O(1) auxiliary space (excluding the output vector).

**Determinism:**
*   Deterministic.

### Mechanism 3: Service Concurrency Wrapper (`NlmService`)

**Intent:**
To provide a thread-safe, asynchronous interface to the `LockRegistry` for the RPC handlers implemented in sub-modules.

**Inputs:**
*   `&self`: Reference to the service instance.

**Outputs:**
*   `tokio::sync::RwLockReadGuard` or `tokio::sync::RwLockWriteGuard`: Guards providing access to the internal `LockRegistry`.

**Steps:**
1.  The struct holds a `tokio::sync::RwLock<LockRegistry>`.
2.  Public methods (or sub-module implementations) acquire this lock. Read operations (like `TEST`) acquire a read lock, allowing concurrency. Write operations (like `LOCK`, `UNLOCK`) acquire a write lock, ensuring exclusive access.

**Edge Cases:**
*   **Lock Contention**: High contention on write locks will cause tasks to await, but the logic itself remains consistent.

**Complexity:**
*   **Time**: Depends on OS scheduler and lock contention; O(1) for lock acquisition in the uncontended case.
*   **Space**: O(1) for the guard wrapper.

**Determinism:**
*   Non-deterministic (due to scheduling/locking order), but the state transitions are deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

*   **From `nfs_mamont::vfs::file`**:
    *   **`Handle`**: This module uses `Handle` as the primary key in its `HashMap`s to index locks. It relies on the `Handle` being a unique identifier for a file instance to correctly scope locks.
*   **From `nfs_mamont::nlm::holder`**:
    *   **`Nlm4Holder`**: The `find_conflict` mechanism constructs and returns a `Nlm4Holder` to report the identity of the conflicting lock owner to the client. This module relies on `Nlm4Holder` to encapsulate the exclusive status, PID, opaque handle, and range of the conflict.
*   **From `nfs_mamont::nlm`**:
    *   **`OpaqueHandle`**: Both `ActiveLock` and `PendingLock` store an `OpaqueHandle`. This module uses it to distinguish lock owners (e.g., in `find_conflict` where it skips locks with the same handle).
*   **From `nfs_mamont::nlm::cookie`**:
    *   **`Cookie`**: The `PendingLock` struct stores a `Cookie`. This module preserves this value so that when the lock is eventually granted (via `grant_pending`), the caller can use the cookie to identify the original transaction in a callback.
*   **From `nfs_mamont::consts::nlm`**:
    *   **`LM_MAXSTRLEN`**: The `check_caller_name` function uses this constant to validate the length of the `caller_name` field in incoming lock requests, ensuring it adheres to protocol limits.

---

## 4. Data Model

Entities:
*   **`ActiveLock`**: Represents a currently held lock.
    *   Fields: `caller_name`, `system_identifier`, `exclusive`, `offset`, `length`, `opaque_handle`.
*   **`PendingLock`**: Represents a lock request that is blocked and waiting.
    *   Fields: `caller_name`, `system_identifier`, `exclusive`, `offset`, `length`, `opaque_handle`, `cookie`.
*   **`LockRegistry`**: The central state container.
    *   Fields: `by_file` (Map of Handle -> Vec<ActiveLock>), `pending` (Map of Handle -> Vec<PendingLock>).
*   **`NlmService`**: The public-facing service struct.
    *   Fields: `locks` (RwLock<LockRegistry>).

Relations:
*   **Composition**: `NlmService` contains a `LockRegistry`.
*   **Aggregation**: `LockRegistry` aggregates `ActiveLock` and `PendingLock` instances, indexed by `Handle`.
*   **Transformation**: `PendingLock` can be converted into `ActiveLock` (via `From` trait), dropping the `cookie`.

Global Invariants:
*   **Unlock Identity**: For `ActiveLock`, equality (`PartialEq`) is determined solely by `caller_name`, `system_identifier`, `offset`, and `length`. The `exclusive` mode and `opaque_handle` are ignored, matching the NLM protocol's definition of an unlock identity.
*   **Cancel Identity**: For `PendingLock`, equality includes `exclusive` and `opaque_handle` but excludes `cookie`.
*   **Range Integrity**: The `merge_adjacent` function ensures that there are no overlapping or adjacent locks for the same owner and mode in the `by_file` list.
*   **EOF Semantics**: A `length` of `0` in any lock struct implies the lock extends to the end of the file.

## 5. Error Model

Error Types:
*   **`std::io::Error`**:
    *   **`InvalidInput`**: Returned by `check_caller_name` if the name is empty or exceeds `LM_MAXSTRLEN`.

Error Propagation Strategy:
*   **Result Types**: Internal methods like `ActiveLock::new`, `PendingLock::new`, `split_lock`, and `LockRegistry` methods return `Result<T, std::io::Error>`.
*   **Mapping**: The RPC handlers in sub-modules (e.g., `lock`, `unlock`) catch these errors and map them to `Nlm4Stats::Failed` in the response.

Recoverability:
*   **Recoverable**: Validation errors (like invalid names) result in immediate failure of the specific RPC call but do not corrupt the `LockRegistry` state.

Panics:
*   **Allowed**: No.
*   **Conditions**: The code uses `expect` in `From<&PendingLock> for ActiveLock` ("PendingLock must have valid caller_name"), implying that if a `PendingLock` exists in the registry, it must have passed validation. This panic serves as an internal invariant check.

---

## 6. Traits

List which external traits this module implements:
*   **`PartialEq` for `ActiveLock`**: Custom implementation comparing only owner and range fields.
*   **`PartialEq` for `PendingLock`**: Custom implementation comparing owner, range, mode, and handle (excluding cookie).
*   **`From<&PendingLock> for ActiveLock`**: Conversion implementation.
*   **`Default` for `NlmService`**: Creates a new service with an empty registry.

---

## 7. Overview

This module is used in order to **provide the core in-memory state management and conflict resolution logic for the Network Lock Manager (NLM) v4 service**. It acts as the "brain" of the locking service, maintaining the authoritative record of which clients hold which locks on which files, and determining whether new lock requests can be granted.

The system contains a distributed file server (NFS) that requires a robust locking mechanism to prevent data corruption. This module implements the specific algorithms required to manage byte-range locks. It handles the complexity of "lock splitting" (when a client unlocks a portion of a larger held lock) and "lock merging" (optimizing storage by combining adjacent locks from the same owner). It also manages the queue of pending requests, ensuring that when a resource becomes free, the correct waiting clients are notified.

A typical usage scenario of the system involves a client requesting a lock. The RPC handler (in a sub-module like `lock.rs`) calls methods on `NlmService`. The service acquires a lock on the registry, checks for conflicts using `find_conflict`, and either grants the lock immediately (adding it to `by_file`) or queues it (adding it to `pending`). Later, when a client unlocks a region, the `unlock` handler calls `remove_by_owner` to modify the active locks and `grant_pending` to see if any queued requests can now be satisfied.

Inside the system, the following things happen and they use this module:
1.  **State Synchronization**: The `NlmService` wraps the `LockRegistry` in a `tokio::sync::RwLock`. This allows the system to safely handle concurrent RPC requests from multiple clients, ensuring that lock state is never corrupted by race conditions.
2.  **Protocol Semantics**: The module implements specific NLM protocol rules, such as the definition of lock equality for unlocking (ignoring mode) and the handling of "to end-of-file" ranges (length 0).
3.  **Resource Management**: By splitting and merging locks, the module ensures that the internal representation of locks remains accurate and efficient, even when clients perform complex sequences of overlapping lock and unlock operations.

Without this module, the NLM service would lack the data structures and algorithms necessary to track locks, making it impossible to enforce locking semantics or prevent data corruption in a multi-client environment.