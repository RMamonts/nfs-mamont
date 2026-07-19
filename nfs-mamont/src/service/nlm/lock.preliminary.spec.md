<!-- SPEC_HASH: a7ade64aad00a0d2248b4024bd9db865b2868123e9959b21e9f6bb2f9defabfd -->
# Module Specification

Module: nfs_mamont::service::nlm::lock
Rust File: src/service/nlm/lock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::procedures::lock::{Lock, Nlm4LockArgs, Nlm4LockRes}`**:
    *   **`Lock`**: The asynchronous trait being implemented. This module provides the concrete server-side logic for the NLM `LOCK` procedure.
    *   **`Nlm4LockArgs`**: The input structure containing the request details (caller, file handle, range, blocking flag, reclaim flag).
    *   **`Nlm4LockRes`**: The output structure used to return the status of the lock operation (granted, denied, blocked) to the client.
*   **`crate::nlm::Nlm4Stats`**:
    *   Used to populate the `stat` field in `Nlm4LockRes`, providing specific status codes such as `Granted`, `Denied`, `Blocked`, or `Failed` based on the logic outcome.
*   **`super::{ActiveLock, NlmService, PendingLock}`**:
    *   **`NlmService`**: The struct on which the `Lock` trait is implemented. It holds the internal state (registry) required to manage locks.
    *   **`PendingLock`**: A type representing a lock request that is either being processed or waiting in a queue. It is constructed from the raw arguments.
    *   **`ActiveLock`**: A type representing a lock that has been successfully granted and is currently active in the registry. It is derived from a `PendingLock`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the core logic of the NLM v4 `LOCK` procedure. This involves validating the request, checking for conflicts with existing locks, managing the transition of a lock from "pending" to "active" state, and handling blocking requests or recovery (reclaim) scenarios.

Inputs:
- `&self`: A reference to the `NlmService` instance, which provides access to the shared lock registry.
- `args: Nlm4LockArgs`: The arguments for the lock request, including the file handle, lock range, exclusive flag, block flag, reclaim flag, and cookie.

Outputs:
- `Nlm4LockRes`: The result structure containing the original `cookie` and a `stat` field indicating the outcome (`Granted`, `Denied`, `Blocked`, or `Failed`).

Steps:
1.  **Acquire Registry Lock**: The method acquires a write lock on `self.locks` (the internal registry) using `.write().await`. This ensures exclusive access to the lock state for the duration of the operation.
2.  **Construct Pending Lock**: It attempts to create a `PendingLock` instance from the arguments (caller name, system ID, exclusive flag, offset, length, opaque handle, cookie). If this construction fails, it returns `Nlm4Stats::Failed`.
3.  **Prepare Active Lock**: It converts the `PendingLock` into an `ActiveLock` representation using `ActiveLock::from(&new_lock)`.
4.  **Conflict Check & Reclaim Handling**:
    *   It checks if the request is a `reclaim` or if there are no conflicts found via `registry.find_conflict(&fh, &new_active_lock)`.
    *   If either condition is true, it attempts to register the lock using `registry.push_or_replace(fh, new_active_lock)`.
    *   If insertion succeeds, it returns `Nlm4Stats::Granted`.
    *   If insertion fails, it returns `Nlm4Stats::Failed`.
5.  **Handle Conflict**:
    *   If a conflict exists and it is not a reclaim request:
        *   If the `block` flag in arguments is `false`, it returns `Nlm4Stats::Denied`.
        *   If the `block` flag is `true`, it pushes the `new_lock` into a pending queue associated with the file handle (`registry.pending.entry(fh).or_default().push(new_lock)`) and returns `Nlm4Stats::Blocked`.

Edge Cases:
- **Invalid Lock Construction**: If `PendingLock::new` returns an `Err`, the operation fails immediately with `Nlm4Stats::Failed`.
- **Registry Insertion Failure**: If `push_or_replace` fails (e.g., due to resource limits or internal inconsistency), the operation fails with `Nlm4Stats::Failed`.
- **Reclaim Mode**: The `reclaim` flag forces the lock to be granted (or attempted) regardless of existing conflicts, typically used during server recovery.

Complexity:
- Time: Unbounded. The time depends on acquiring the write lock on the registry and the complexity of `find_conflict` (which likely depends on the number of existing locks for the file).
- Space: O(1) for the immediate operation, though it adds one entry to either the active locks or the pending queue in the registry.

Determinism:
- Non-deterministic. The result depends on the current state of the registry (which is modified by other concurrent tasks) and the success of acquiring the internal lock.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::nlm::procedures::lock`**:
    - **`Lock` Trait**: Defines the asynchronous interface `async fn lock(&self, args: Nlm4LockArgs) -> Nlm4LockRes`. This module provides the implementation for `NlmService`.
    - **`Nlm4LockArgs`**: Provides the `block`, `reclaim`, `exclusive`, and `lock` (file handle, offset, length) fields used to drive the logic.
    - **`Nlm4LockRes`**: The container for the return value. The `cookie` must be echoed back, and `stat` is set based on the logic.
- **From `nfs_mamont::nlm`**:
    - **`Nlm4Stats`**: The enum variants `Granted`, `Denied`, `Blocked`, and `Failed` are used to signal the specific outcome of the lock request to the client.

---

## 4. Data Model

Entities:
- **`NlmService`**: The service context. It contains a `locks` field (inferred type) which acts as the registry for both active and pending locks.
- **`PendingLock`**: A transient entity representing a lock request. It holds the details provided by the client (caller, handle, range) before it is granted.
- **`ActiveLock`**: An entity representing a lock that is currently held. It is derived from `PendingLock` and stored in the registry.
- **`LockRegistry` (Inferred)**: The internal state accessed via `self.locks`. It appears to be a map-like structure keyed by file handle, containing collections of `ActiveLock`s and `PendingLock`s. It exposes methods `write()`, `find_conflict`, `push_or_replace`, and provides access to a `pending` collection.

Relations:
- `NlmService` *aggregates* `LockRegistry`.
- `PendingLock` *is converted to* `ActiveLock`.
- `LockRegistry` *stores* `ActiveLock` instances.
- `LockRegistry` *queues* `PendingLock` instances.

Global Invariants:
- **Mutual Exclusion**: The `write().await` access to the registry ensures that lock state modifications are atomic.
- **Conflict Resolution**: An `ActiveLock` cannot coexist with a conflicting `ActiveLock` unless the `reclaim` flag is set.
- **Queueing**: If a lock request cannot be granted and `block` is true, it must be stored in the `pending` queue, and the status must be `Blocked`.

---

## 5. Error Model

Error Types:
- No explicit Rust error types (like `Result::Err`) are returned directly. Errors are encoded in the `Nlm4Stats` enum.

Error Propagation Strategy:
- **Status Codes**: The logic maps different failure conditions to specific `Nlm4Stats` variants.
    - `PendingLock::new` failure -> `Nlm4Stats::Failed`.
    - `registry.push_or_replace` failure -> `Nlm4Stats::Failed`.
    - Conflict found + `block=false` -> `Nlm4Stats::Denied`.

Recoverability:
- **`Denied`**: The client may retry the request later, assuming the conflicting lock might be released.
- **`Blocked`**: The client waits for a callback (GRANTED) from the server.
- **`Failed`**: Indicates a server-side issue (e.g., allocation failure). Retrying immediately is unlikely to succeed.

Panics:
- Allowed: No.
- Conditions: The code performs defensive checks (match on `PendingLock::new`, check `push_or_replace` result). However, a panic could occur if `self.locks` is poisoned or if the internal registry methods panic on invalid state.

---

## 6. Traits

List which external traits this module implements:
- **`crate::nlm::procedures::lock::Lock`**: Implemented for `NlmService`. This provides the asynchronous `lock` method that handles the NLM protocol logic.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to enforce the synchronization semantics of the Network Lock Manager (NLM) protocol within the NFS server. It acts as the state machine engine that decides whether a file lock request from a client should be granted, denied, or queued, based on the current state of the system and the rules of the protocol.

The system contains the concrete implementation of the `Lock` trait, bridging the gap between the abstract protocol definitions (arguments and results) and the actual in-memory state of the lock manager. It ensures that access to shared resources (files) is coordinated correctly among multiple clients, preventing data corruption due to concurrent writes.

A typical usage scenario of the system involves an NFS client requesting an exclusive lock on a file. The system receives this request via the `Nlm4LockArgs`, checks the internal registry for conflicting locks. If the coast is clear, it records the lock and confirms the grant. If another client holds the lock, the system either denies the request immediately or queues it (if the client requested to block), ensuring that the client is notified later when the resource becomes available.

Inside the system the following things happen and they use a write-ahead locking mechanism on the internal registry to ensure thread safety. The system differentiates between "active" locks (currently enforced) and "pending" locks (waiting for enforcement). It also handles special recovery scenarios (`reclaim` flag) where locks are restored after a server restart without checking for conflicts, which is essential for maintaining state consistency across server reboots in distributed environments.