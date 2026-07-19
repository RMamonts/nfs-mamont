<!-- SPEC_HASH: a7ade64aad00a0d2248b4024bd9db865b2868123e9959b21e9f6bb2f9defabfd -->
# Module Specification

Module: nfs_mamont::service::nlm::lock
Rust File: src/service/nlm/lock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::procedures::lock`**:
    *   Used to provide the `Lock` trait, which defines the asynchronous interface `lock` that this module implements for `NlmService`.
    *   Used to provide `Nlm4LockArgs` (input structure containing lock details, flags, and cookie) and `Nlm4LockRes` (output structure containing the status and cookie).
*   **`crate::nlm`**:
    *   Used to provide the `Nlm4Stats` enum, specifically the variants `Granted`, `Denied`, `Blocked`, and `Failed`, which are used to populate the `stat` field of the response.
*   **`crate::service::nlm`** (parent module):
    *   Used to access `NlmService`, the struct for which the `Lock` trait is being implemented.
    *   Used to access `ActiveLock` and `PendingLock`, which represent the lock state in memory.
    *   Used to access the internal `LockRegistry` (via `self.locks`), which provides the methods `find_conflict`, `push_or_replace`, and access to the `pending` queue.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the `Lock` procedure for the `NlmService`. This involves validating the request, checking for conflicts with existing locks, handling crash recovery (reclaims), and managing the queue of blocked requests according to the NLMv4 protocol.

Inputs:
- **`&self`**: A reference to the `NlmService` instance, which holds the `LockRegistry`.
- **`args: Nlm4LockArgs`**: The arguments for the lock request, including:
    - `cookie`: Transaction identifier.
    - `block`: Boolean indicating if the client wants to wait for the lock.
    - `exclusive`: Boolean indicating if the lock is exclusive or shared.
    - `lock`: `Nlm4Lock` struct containing `file_handle`, `caller_name`, `owner`, `offset`, and `length`.
    - `reclaim`: Boolean indicating if this is a recovery request.
    - `state`: NSM state (unused in this specific logic but present in args).

Outputs:
- **`Nlm4LockRes`**: The result structure containing:
    - `cookie`: The echoed transaction identifier.
    - `stat`: The status code (`Granted`, `Denied`, `Blocked`, or `Failed`).

Steps:
1.  **Acquire Lock**: The method acquires a write lock on `self.locks` (the `LockRegistry`) using `write().await`. This ensures exclusive access to the lock state for the duration of the operation.
2.  **Construct Request**: It attempts to create a `PendingLock` from the arguments using `PendingLock::new`.
    - If this fails (e.g., invalid caller name), it immediately returns `Nlm4LockRes` with `stat: Nlm4Stats::Failed`.
3.  **Prepare for Check**: It extracts the file handle (`fh`) and converts the `PendingLock` into an `ActiveLock` representation (`new_active_lock`) to perform conflict detection against currently held locks.
4.  **Conflict Check & Reclaim**:
    - It checks if the request is a `reclaim` OR if `registry.find_conflict(&fh, &new_active_lock)` returns `None`.
    - If true (no conflict or reclaiming):
        - It calls `registry.push_or_replace(fh, new_active_lock)` to insert the lock.
        - If insertion fails, it returns `Nlm4Stats::Failed`.
        - Otherwise, it returns `Nlm4Stats::Granted`.
5.  **Handle Conflict**:
    - If a conflict exists and it is not a reclaim:
        - It checks the `args.block` flag.
        - If `args.block` is `false` (non-blocking), it returns `Nlm4Stats::Denied`.
        - If `args.block` is `true` (blocking):
            - It accesses `registry.pending`, gets the vector for the file handle (creating a default one if necessary), and pushes the `new_lock` (the `PendingLock`) onto it.
            - It returns `Nlm4Stats::Blocked`.

Edge Cases:
- **Invalid Arguments**: If `PendingLock::new` returns an `Err`, the operation fails immediately with `Nlm4Stats::Failed` without modifying the registry.
- **Reclaim Mode**: If `args.reclaim` is true, the lock is granted even if conflicts exist (bypassing `find_conflict`), assuming the server is in a grace period or the client is restoring state.
- **Registry Insertion Failure**: If `push_or_replace` returns an `Err`, the status is `Failed` even if no conflict was found.

Complexity:
- **Time**: Dominated by the `LockRegistry` operations. `find_conflict` is O(N) where N is the number of active locks on the file. `push_or_replace` is O(N) due to merging/splitting logic. Acquiring the `RwLock` is dependent on system contention.
- **Space**: O(1) for the immediate function scope, though the registry grows with the number of active and pending locks.

Determinism:
- **Deterministic**. Given the same state of `LockRegistry` and the same input arguments, the output and state transitions are identical.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::service::nlm`**:
    - **`LockRegistry::find_conflict`**: Used to determine if the requested lock range overlaps with any existing lock held by a different owner. This is the core decision-making factor for granting vs. denying the lock.
    - **`LockRegistry::push_or_replace`**: Used to persist the new lock in the active registry. This method handles the logic of replacing existing locks owned by the same client and merging adjacent ranges.
    - **`LockRegistry::pending`**: A map accessed to store the `PendingLock` when the request cannot be granted immediately and the client requested to block.
    - **`tokio::sync::RwLock`**: The `write().await` call on `self.locks` is used to ensure that no other thread modifies the lock registry (active or pending) while this procedure is running.

- **From `nfs_mamont::nlm::procedures::lock`**:
    - **`Lock` trait**: Defines the signature `async fn lock(&self, args: Nlm4LockArgs) -> Nlm4LockRes` which this module implements.
    - **`Nlm4LockArgs`**: Provides the `block` and `reclaim` flags which control the control flow (whether to queue the request or bypass conflict checks).

- **From `nfs_mamont::nlm`**:
    - **`Nlm4Stats`**: Provides the enumeration values (`Granted`, `Denied`, `Blocked`, `Failed`) that signal the outcome of the operation to the client.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It manipulates entities defined in its dependencies:
    - **`PendingLock`**: A temporary representation of the lock request used for validation and queuing.
    - **`ActiveLock`**: The representation of a lock used for conflict detection and storage in the active registry.

Relations:
- **`Nlm4LockArgs` -> `PendingLock`**: The arguments are transformed into a `PendingLock` to validate and hold the request details.
- **`PendingLock` -> `ActiveLock`**: The `PendingLock` is converted into an `ActiveLock` to check against the registry.
- **`NlmService` -> `LockRegistry`**: The service owns the registry which stores both `ActiveLock`s and `PendingLock`s.

Global Invariants:
- **Queue Consistency**: A lock is added to `registry.pending` only if `args.block` is true and a conflict exists.
- **Reclaim Priority**: If `args.reclaim` is true, the lock is added to the active registry regardless of conflicts.

## 5. Error Model

Error Types:
- **`Nlm4Stats::Failed`**: Used to indicate a failure in constructing the lock request (e.g., invalid parameters) or a failure in inserting the lock into the registry.

Error Propagation Strategy:
- **Mapping to Status**: The method does not return a Rust `Result`. Instead, internal errors (like `Err` from `PendingLock::new` or `push_or_replace`) are caught and mapped to the `Nlm4Stats::Failed` variant in the returned `Nlm4LockRes`.

Recoverability:
- **Client-Side**: The client must interpret the `Failed` status. For `Denied` or `Blocked`, the client protocol dictates retrying or waiting for a callback. `Failed` typically indicates a fatal error in the request or server state that prevents the operation from being attempted.

Panics:
- **Allowed**: No explicit panics in this code.
- **Conditions**: If the `RwLock` protecting `self.locks` is poisoned (e.g., a previous thread panicked while holding the lock), the `write().await` call will panic.

---

## 6. Traits

List which external traits this module implements:
- **`crate::nlm::procedures::lock::Lock`**: Implemented for `crate::service::nlm::NlmService`.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **implement the server-side logic for the NLMv4 `LOCK` procedure**, effectively deciding whether a client's request to lock a file region is accepted, denied, or queued. It serves as the bridge between the abstract protocol definition (the `Lock` trait) and the concrete, thread-safe state management (the `LockRegistry`).

The system contains a distributed lock manager where `NlmService` acts as the central authority. This module provides the specific decision-making algorithm for lock acquisition. It interprets the client's intent (blocking vs. non-blocking, recovering vs. new request) and applies it to the current state of the system. It ensures that the invariants of the lock registry (no overlapping conflicting locks) are maintained while handling the asynchronous nature of the NLM protocol (queuing blocked requests).

A typical usage scenario of the system involves a client requesting an exclusive lock on a file. The RPC layer deserializes this into `Nlm4LockArgs` and calls the `lock` method implemented here. The module acquires exclusive access to the registry, checks if the file is already locked by another client, and finds a conflict. If the client requested a blocking lock, the module serializes the request into a `PendingLock` and stores it in the registry's pending queue, returning `Blocked`. The server will later notify the client when the lock becomes available. If the client requested a non-blocking lock, the module returns `Denied` immediately.

Inside the system, the following things happen and they use this module:
1.  **State Synchronization**: The module uses `self.locks.write().await` to ensure that checking for conflicts and modifying the lock state happen atomically. This prevents race conditions where two clients might believe they both acquired a lock simultaneously.
2.  **Recovery Handling**: By checking the `reclaim` flag, this module allows the server to bypass conflict detection during recovery phases, allowing clients to restore their state after a server restart without being denied by locks that haven't been re-established yet.
3.  **Queue Management**: It is responsible for populating the `pending` queue in the `LockRegistry`. This queue is later consumed by other procedures (like `Unlock`) to grant locks to waiting clients when resources become free.

Without this module, the `NlmService` would have the data structures to hold locks but no logic to determine *how* and *when* to accept them, rendering the locking service non-functional.