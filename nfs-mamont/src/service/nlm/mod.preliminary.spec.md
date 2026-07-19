<!-- SPEC_HASH: 9bd5d2a2d117e6be85a2a83924763544cf89419a6bd4b5424c373f262e346b7a -->
# Module Specification

Module: nfs_mamont::service::nlm
Rust File: src/service/nlm/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`std::collections::HashMap`**:
    *   Used in `LockRegistry` to store active locks (`by_file`) and pending lock requests (`pending`), indexed by `vfs::file::Handle`. This provides O(1) average access to the lock list for a specific file.
*   **`std::io::Error`**:
    *   Used as the error type for validation functions (e.g., `check_caller_name`) and constructors (`ActiveLock::new`, `PendingLock::new`) to indicate invalid input parameters.
*   **`tokio::sync::RwLock`**:
    *   Used in `NlmService` to wrap the `LockRegistry`. This allows concurrent read access (for `TEST` operations) while ensuring exclusive write access (for `LOCK`, `UNLOCK`, `CANCEL` operations) to the lock state.
*   **`crate::consts::nlm`**:
    *   Used to access the `LM_MAXSTRLEN` constant, which defines the maximum allowed length for the `caller_name` field in lock requests.
*   **`crate::nlm::cookie`**:
    *   Used to provide the `Cookie` type stored in `PendingLock`, which identifies the specific blocking lock request for callback purposes.
*   **`crate::nlm::holder`**:
    *   Used to provide the `Nlm4Holder` type, which is returned by `LockRegistry::find_conflict` to describe the owner of a conflicting lock.
*   **`crate::nlm::OpaqueHandle`**:
    *   Used as a field in `ActiveLock` and `PendingLock` to uniquely identify the lock owner (client-side handle).
*   **`crate::vfs::file::Handle`**:
    *   Used as the key type in the `LockRegistry`'s internal maps to associate locks with specific files.
*   **`crate::service::nlm::{lock, unlock, test, cancel}`**:
    *   These sub-modules implement the NLM procedure traits (`Lock`, `Unlock`, `Test`, `Cancel`) for `NlmService`. They rely on the `LockRegistry` and state management logic defined in this module to perform the actual lock operations.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a thread-safe, in-memory registry for managing file locks (active and pending) with support for range-based conflict detection, lock splitting/merging, and queue management for blocked requests.

Inputs:
- **`NlmService::new()`**: No inputs.
- **`LockRegistry` methods**:
    - `file_handle: &Handle`: The target file.
    - `request: &ActiveLock` or `target: &PendingLock`: The lock details to check, add, or remove.
    - `caller_name: &str`, `system_identifier: i32`, `offset: u64`, `len: u64`: Specific fields for identifying locks to remove.
- **Helper functions**:
    - `start1, len1, start2, len2`: Byte ranges for overlap calculation.
    - `lock: ActiveLock`, `unlock_start, unlock_len`: Parameters for splitting a lock range.

Outputs:
- **`NlmService`**: A new instance of the service.
- **`LockRegistry` operations**:
    - `bool`: For `has_active_lock` and `remove_pending` (indicating existence or success).
    - `Option<Nlm4Holder>`: For `find_conflict` (details of the conflicting lock).
    - `Result<(), Error>`: For `remove_by_owner`, `push_or_replace`, `grant_pending`.
    - `Vec<PendingLock>`: For `grant_pending` (list of locks that were just granted).
- **Helper functions**:
    - `bool`: For `ranges_overlap`.
    - `u64`: For `calculate_end_of_interval`.
    - `Vec<ActiveLock>`: For `split_lock` (fragments of a lock).

Steps:
1.  **Validation (`check_caller_name`)**:
    - Checks if the input string is empty.
    - Checks if the string length exceeds `LM_MAXSTRLEN`.
    - Returns `Error` if either check fails.
2.  **Conflict Detection (`find_conflict`)**:
    - Retrieves the list of active locks for the given file handle.
    - Iterates through the list.
    - Skips locks owned by the same `(caller_name, system_identifier, opaque_handle)`.
    - Skips if both the request and the existing lock are shared (non-exclusive).
    - Checks if the ranges overlap using `ranges_overlap`.
    - Returns `Some(Nlm4Holder)` if a conflict is found, `None` otherwise.
3.  **Lock Insertion (`push_or_replace`)**:
    - Retrieves or creates the list of active locks for the file.
    - Calls `drain_overlapping` to remove any existing locks owned by the same client that overlap with the new range. This prevents duplicate locks and handles upgrades/downgrades.
    - Pushes the new lock to the list.
    - Calls `merge_adjacent` to combine contiguous or overlapping locks from the same owner.
4.  **Lock Removal (`remove_by_owner`)**:
    - Retrieves the list of active locks.
    - Calls `drain_overlapping` with the owner's identity and the range to unlock.
    - `drain_overlapping` iterates the list, and for matching locks, calls `split_lock` to calculate the remaining fragments (left and right of the unlocked range) and extends the list with these fragments.
    - Removes the file entry from the map if no locks remain.
5.  **Pending Queue Management (`grant_pending`)**:
    - Removes the list of pending requests for the file.
    - Iterates through each pending request.
    - Checks if it can be granted using `find_conflict`.
    - If no conflict, converts `PendingLock` to `ActiveLock` and inserts it via `push_or_replace`. Adds it to the `granted` list.
    - If conflict exists, keeps it in the `still_pending` list.
    - Re-inserts any `still_pending` requests back into the registry.
6.  **Range Arithmetic**:
    - `calculate_end_of_interval`: Returns `u64::MAX` if length is `0` (EOF), otherwise `start + len - 1`.
    - `ranges_overlap`: Returns true if intervals `[start1, end1]` and `[start2, end2]` intersect.
    - `split_lock`: Calculates the non-overlapping parts of a lock before and after a given unlock range.
    - `merge_adjacent`: Sorts locks by owner and offset, then merges adjacent or overlapping ranges of the same owner and mode.

Edge Cases:
- **Length 0**: Interpreted as "to end-of-file" (`u64::MAX`) in range calculations.
- **Unlocking non-existent ranges**: `remove_by_owner` handles this gracefully by simply not finding matches.
- **Splitting locks**: Unlocking the middle of a large lock results in two smaller locks (left and right fragments).
- **Merging**: Adjacent locks (e.g., `[0,5)` and `[5,10)`) are merged into `[0,10)`.

Complexity:
- **Time**:
    - `find_conflict`, `remove_by_owner`, `push_or_replace`: O(N) where N is the number of active locks on the specific file.
    - `grant_pending`: O(M * (N + K)) where M is the number of pending requests, N is active locks, and K is the cost of insertion.
    - `merge_adjacent`: O(N log N) due to sorting.
- **Space**:
    - `LockRegistry`: O(T) where T is the total number of active and pending locks across all files.

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
    - **`Handle`**: Used as the primary key in `LockRegistry` maps to group locks by file. It is assumed to be a comparable, hashable identifier.
- **From `nfs_mamont::nlm::holder`**:
    - **`Nlm4Holder`**: Used as the return type for `find_conflict`. The module constructs this struct to describe the conflicting lock to the caller.
- **From `nfs_mamont::nlm::cookie`**:
    - **`Cookie`**: Stored in `PendingLock`. It is assumed to be a simple identifier used to correlate requests with callbacks.
- **From `nfs_mamont::service::nlm::lock`**:
    - **`Lock` trait implementation**: The `lock` module uses `NlmService` (specifically the `LockRegistry` inside it) to check conflicts and add locks. It relies on `push_or_replace` and `find_conflict`.
- **From `nfs_mamont::service::nlm::unlock`**:
    - **`Unlock` trait implementation**: The `unlock` module uses `remove_by_owner` to release locks and `grant_pending` to process the waiting queue.
- **From `nfs_mamont::service::nlm::test`**:
    - **`Test` trait implementation**: The `test` module uses `find_conflict` to check if a lock *would* conflict without modifying state.
- **From `nfs_mamont::service::nlm::cancel`**:
    - **`Cancel` trait implementation**: The `cancel` module uses `remove_pending` to cancel a blocked request and `has_active_lock` to check if the lock was granted in the meantime.

---

## 4. Data Model

Entities:
- **`ActiveLock`**: Represents a currently granted lock. Contains `caller_name`, `system_identifier`, `exclusive` flag, `offset`, `length`, and `opaque_handle`.
- **`PendingLock`**: Represents a lock request that is blocked waiting for a resource. Contains the same fields as `ActiveLock` plus a `cookie`.
- **`LockRegistry`**: The core state container. Holds two maps: `by_file` (maps `Handle` to `Vec<ActiveLock>`) and `pending` (maps `Handle` to `Vec<PendingLock>`).
- **`NlmService`**: The public-facing service struct. Contains a `tokio::sync::RwLock<LockRegistry>` to manage access to the registry.

Relations:
- **`NlmService` owns `LockRegistry`**: The registry is the internal state of the service.
- **`LockRegistry` indexes `ActiveLock` by `Handle`**: One file can have multiple active locks.
- **`LockRegistry` indexes `PendingLock` by `Handle`**: One file can have multiple pending requests.
- **`PendingLock` converts to `ActiveLock`**: When a pending lock is granted, it is transformed into an active lock.

Global Invariants:
- **No Conflicting Active Locks**: For any file, there must be no two `ActiveLock`s with different owners where one is exclusive and their ranges overlap.
- **Pending Implies Conflict**: Any lock in the `pending` queue must conflict with at least one lock in the `active` list for that file.
- **Owner Consistency**: `ActiveLock` and `PendingLock` for the same logical lock must share `caller_name`, `system_identifier`, and `opaque_handle`.

## 5. Error Model

Error Types:
- **`std::io::Error`**:
    - Returned by `check_caller_name`, `ActiveLock::new`, and `PendingLock::new` if the `caller_name` is empty or exceeds `LM_MAXSTRLEN`.

Error Propagation Strategy:
- **Propagation via `Result`**: Internal functions return `Result<(), Error>`. The `NlmService` methods (implemented in sub-modules) catch these and map them to NLM status codes (e.g., `Nlm4Stats::Failed`).

Recoverability:
- **Validation Errors**: Recoverable by the client (fix the request).
- **Registry Errors**: Generally unrecoverable within the transaction (indicates internal state corruption or logic error), resulting in a `Failed` status to the client.

Panics:
- **Allowed**: No.
- **Conditions**: The code uses `expect` in `From<&PendingLock> for ActiveLock` ("PendingLock must have valid caller_name"), which assumes that a `PendingLock` can only exist if it was constructed successfully. If a `PendingLock` with an invalid name were somehow created manually, this would panic.

---

## 6. Traits

List which external traits this module implements:
- **`Default`**: Implemented for `NlmService` (delegates to `new`).
- **`PartialEq`**: Implemented for `ActiveLock` (compares identity fields: caller, system ID, offset, length) and `PendingLock` (compares identity fields plus exclusive flag and handle).
- **`From<&PendingLock> for ActiveLock`**: Allows converting a pending request into an active lock representation.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **provide the core in-memory state management and conflict resolution logic for the Network Lock Manager (NLM) v4 service**. It acts as the authoritative source of truth for which files are locked, by whom, and in what ranges, within the NFS server.

The system contains the `NlmService`, which wraps a `LockRegistry`. This registry is responsible for the complex algorithms required to manage byte-range locks. It handles the "plumbing" of lock management: determining if two locks overlap (`ranges_overlap`), splitting a lock when a portion of it is unlocked (`split_lock`), merging adjacent locks to optimize storage (`merge_adjacent`), and managing the queue of clients waiting for a lock (`grant_pending`).

A typical usage scenario of the system involves an NFS client requesting a lock via the `LOCK` procedure. The request is handled by the `lock` submodule (which implements the RPC trait), which in turn calls methods on the `NlmService`. The `NlmService` acquires a write lock on the registry and uses `find_conflict` to check if the request is valid. If valid, it uses `push_or_replace` to record the lock. If another client holds a conflicting lock, the request is added to the `pending` queue. Later, when the first client unlocks the file, the `unlock` submodule calls `remove_by_owner` followed by `grant_pending`. The registry then identifies that the waiting client can now proceed, moves the lock to the active list, and returns the details so the server can send a `GRANTED` callback.

Inside the system, the following things happen and they use this module: The `NlmService` is instantiated once in the main server loop (`lib.rs`) and shared via an `Arc`. The sub-modules (`lock`, `unlock`, `test`, `cancel`) use the public interface of `NlmService` (accessing the internal `LockRegistry` via the `RwLock`) to perform their specific protocol duties. This separation ensures that the protocol handling (RPC arguments/results) is decoupled from the state management algorithms (range arithmetic, conflict detection). Without this module, the NLM service would lack a centralized, thread-safe mechanism to enforce locking semantics, leading to potential data corruption and race conditions.