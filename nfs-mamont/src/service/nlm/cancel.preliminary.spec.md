<!-- SPEC_HASH: 9f1c6550a09c30d42f2459ebf901bccb5afa061549204c56ba6bfc5839dd1b2a -->
# Module Specification

Module: nfs_mamont::service::nlm::cancel
Rust File: src/service/nlm/cancel.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::procedures::cancel::{Cancel, Nlm4CancelArgs, Nlm4CancelRes}`**:
    *   Used to define the interface that this module implements. `Cancel` is the asynchronous trait defining the service contract, while `Nlm4CancelArgs` and `Nlm4CancelRes` are the input and output types for the procedure.
*   **`crate::nlm::Nlm4Stats`**:
    *   Used to populate the `stat` field in the response, indicating the outcome of the cancellation attempt (e.g., `Granted`, `Denied`, `Failed`).
*   **`super::{ActiveLock, NlmService, PendingLock}`**:
    *   `NlmService` is the struct implementing the `Cancel` trait.
    *   `PendingLock` is used to construct a canonical representation of the lock request from the arguments to facilitate lookup in the registry.
    *   `ActiveLock` is used to check if the lock has already been granted, as the implementation returns `Granted` if the lock is active.
*   **`crate::nlm::lock::Nlm4Lock`** (Implicit via `args.lock`):
    *   Used to access specific lock details such as `caller_name`, `system_identifier`, `file_handle`, `lock_offset`, `lock_length`, and `opaque_handle` required to construct the `PendingLock`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide the concrete implementation of the NLMv4 `CANCEL` procedure for the `NlmService`. This implementation attempts to remove a blocked lock request from the internal registry or verifies if the lock is already active.

Inputs:
- `self`: A reference to the `NlmService` instance.
- `args`: `Nlm4CancelArgs` containing the `cookie`, `exclusive` flag, and `Nlm4Lock` details identifying the lock to cancel.

Outputs:
- `Nlm4CancelRes`: A structure containing the echoed `cookie` and a `stat` (`Nlm4Stats`) indicating the result.

Steps:
1.  **Lock Construction**: The method attempts to construct a `PendingLock` instance using fields from `args.lock` (caller name, system ID, offset, length, opaque handle) and `args.cookie` and `args.exclusive`.
2.  **Validation**: If `PendingLock::new` returns an `Err`, the method immediately returns `Nlm4CancelRes` with `stat` set to `Nlm4Stats::Failed`.
3.  **Registry Lock Acquisition**: The method acquires a write lock on `self.locks` (assumed to be a `RwLock` or similar async synchronization primitive) to gain exclusive access to the lock registry.
4.  **Pending Removal**: It calls `registry.remove_pending(&fh, &target)`, passing the file handle and the constructed `PendingLock`.
    *   If this returns `true`, the lock was successfully removed from the pending queue. The method returns `Nlm4CancelRes` with `stat` set to `Nlm4Stats::Granted`.
5.  **Active Check**: If the lock was not found in the pending queue, the method converts the `PendingLock` (`target`) into an `ActiveLock` using `Into` trait.
6.  **Active Verification**: It calls `registry.has_active_lock(&fh, &request_as_active)`.
    *   If this returns `true`, indicating the lock is currently held, the method returns `Nlm4CancelRes` with `stat` set to `Nlm4Stats::Granted`.
7.  **Denial**: If the lock is neither pending nor active, the method returns `Nlm4CancelRes` with `stat` set to `Nlm4Stats::Denied`.

Edge Cases:
- **Invalid Lock Arguments**: If the arguments provided cannot be used to construct a valid `PendingLock` (e.g., invalid ranges or handles), the constructor fails, resulting in a `Failed` status.
- **Lock State Transition**: If a lock transitions from pending to active between the time the request is received and the registry lock is acquired, the logic handles this by checking the active state after failing to find a pending lock.

Complexity:
- Time: O(1) or O(log N) depending on the underlying data structure of the lock registry (not visible here, but implied by the lookup/remove methods).
- Space: O(1) additional space for the `PendingLock` and `ActiveLock` instances created during the call.

Determinism:
- Deterministic. The result depends solely on the state of the `NlmService` registry and the input arguments.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::nlm::procedures::cancel`**:
    - `Cancel` trait: Defines the `async fn cancel(&self, args: Nlm4CancelArgs) -> Nlm4CancelRes` signature that this module implements.
    - `Nlm4CancelArgs`: Provides the `cookie`, `block`, `exclusive`, and `lock` fields.
    - `Nlm4CancelRes`: Provides the structure for the response, specifically requiring the `cookie` to be echoed back and the `stat` to be set.
- **From `nfs_mamont::nlm`**:
    - `Nlm4Stats`: Enumerates the status codes used in the response (`Granted`, `Denied`, `Failed`).
- **From `nfs_mamont::service::nlm` (Assumptions based on usage)**:
    - `NlmService`: Acts as the container for the lock state. It is assumed to have a `locks` field.
    - `NlmService.locks`: Assumed to be an asynchronous lock-aware container (e.g., `tokio::sync::RwLock`) guarding the internal registry. It exposes a `write().await` method returning a guard with mutable access.
    - `Registry Guard`: The type returned by `self.locks.write()` is assumed to have methods `remove_pending(&FileHandle, &PendingLock) -> bool` and `has_active_lock(&FileHandle, &ActiveLock) -> bool`.
    - `PendingLock`: Assumed to have a `new` constructor that takes lock details and a cookie, returning a `Result`.
    - `ActiveLock`: Assumed to be constructible from a reference to a `PendingLock` (via `impl From<&PendingLock> for ActiveLock`).

---

## 4. Data Model

Entities:
- `NlmService`: The service implementation holding the state.
- `PendingLock`: A transient entity representing a lock request waiting to be granted.
- `ActiveLock`: A transient entity representing a currently granted lock.
- `Registry`: The internal state (accessed via `self.locks`) managing the sets of pending and active locks.

Relations:
- `NlmService` *contains* `Registry` (via `locks` field).
- `PendingLock` *is converted to* `ActiveLock` for verification purposes.

Global Invariants:
- The `cookie` in the response must match the `cookie` in the request.
- Access to the registry is mutually exclusive during the execution of this method (enforced by `write().await`).

## 5. Error Model

Error Types:
- None explicitly defined in this module. Errors are communicated via the `Nlm4Stats` enum in the response.

Error Propagation Strategy:
- Status codes via `Nlm4Stats`.
    - `Failed`: Returned if `PendingLock::new` fails (e.g., invalid arguments).
    - `Denied`: Returned if the lock is neither pending nor active.
    - `Granted`: Returned if the lock was successfully removed from pending or if it is found to be active.

Recoverability:
- The client determines recoverability based on the `stat` code. A `Failed` status indicates a server-side issue with the request format. A `Denied` status indicates the lock did not exist in the pending queue (and was not active).

Panics:
- Allowed: No.
- Conditions: The code uses `match` on the result of `PendingLock::new`, preventing panics from unwrapping. Panics would only occur if the internal `self.locks` lock is poisoned or if the dependency methods (`remove_pending`, `has_active_lock`) panic.

---

## 6. Traits

List which external traits this module implements:
- `crate::nlm::procedures::cancel::Cancel`: Implemented for `NlmService`.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to implement the server-side logic for the NLM (Network Lock Manager) CANCEL procedure within the `NlmService`. The NLM protocol requires a mechanism for clients to withdraw lock requests that are currently blocked (waiting for a conflicting lock to be released). This module bridges the abstract protocol definition (arguments and results defined in `nlm::procedures::cancel`) with the concrete state management of the `NlmService`.

This system contains the logic required to interpret a cancellation request, normalize the request data into a `PendingLock` object, and safely modify the service's internal lock registry. It ensures that operations on the registry are thread-safe (via asynchronous write locks) and handles various edge cases, such as invalid request data or locks that have already transitioned from a pending state to an active state.

A typical usage scenario of the system involves a client that previously requested a lock which was blocked. If the client application times out or the user cancels the operation, the client sends a CANCEL request to the server. The `NlmService` receives this request, and the implementation in this module executes. It checks if the lock is still waiting in the pending queue; if so, it removes it and reports success. If the lock has already been granted (active) in the interim, this specific implementation also reports success (`Granted`). If the lock cannot be found in either state, it reports `Denied`.

Inside the system the following things happen and they use the `Cancel` trait to abstract the RPC handling details from the business logic of lock management. The `NlmService` uses its internal `locks` registry (assumed to be a map or set guarded by a `RwLock`) to persist the state of locks across asynchronous RPC calls. The conversion from `PendingLock` to `ActiveLock` allows the system to reuse the matching logic for both waiting and granted locks.