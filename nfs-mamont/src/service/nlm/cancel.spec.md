<!-- SPEC_HASH: 9f1c6550a09c30d42f2459ebf901bccb5afa061549204c56ba6bfc5839dd1b2a -->
# Module Specification

Module: nfs_mamont::service::nlm::cancel
Rust File: src/service/nlm/cancel.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::procedures::cancel`**:
    *   Used to import the `Cancel` trait, `Nlm4CancelArgs`, and `Nlm4CancelRes`. This module provides the implementation of the `Cancel` trait for `NlmService`, bridging the generic RPC interface with the specific lock management logic.
*   **`crate::nlm::Nlm4Stats`**:
    *   Used to populate the `stat` field in `Nlm4CancelRes`. It provides the standardized status codes (`Granted`, `Denied`, `Failed`) required by the NLM protocol to indicate the outcome of the cancellation attempt.
*   **`super` (crate::service::nlm)**:
    *   Used to access `NlmService` (the struct implementing the trait), `PendingLock` (to construct a search key for the request), `ActiveLock` (to check if the lock is already granted), and the internal `LockRegistry` (via `self.locks`) to modify the lock state.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the server-side logic for the NLMv4 `CANCEL` procedure. This involves verifying if a specific lock request is currently blocked (pending) or already granted (active), and modifying the lock registry or reporting status accordingly.

Inputs:
- **`&self`**: A reference to the `NlmService` instance, providing access to the `LockRegistry`.
- **`args: Nlm4CancelArgs`**: The arguments from the RPC call, containing the `cookie`, `exclusive` flag, and the `lock` details (caller, file handle, range, owner).

Outputs:
- **`Nlm4CancelRes`**: The result structure containing the echoed `cookie` and a `stat` (`Nlm4Stats`) indicating success (`Granted`), failure (`Denied`), or error (`Failed`).

Steps:
1.  **Target Construction**: Attempts to construct a `PendingLock` instance from the arguments (`caller_name`, `system_identifier`, `exclusive`, `offset`, `length`, `opaque_handle`, `cookie`). This step validates the input parameters (e.g., caller name length).
2.  **Validation Handling**: If `PendingLock::new` returns an `Err`, the function immediately returns `Nlm4CancelRes` with `stat: Nlm4Stats::Failed`.
3.  **Registry Lock Acquisition**: Acquires a write lock on `self.locks` (the `tokio::sync::RwLock<LockRegistry>`) to ensure exclusive access to the lock state during the check and potential modification.
4.  **Pending Lock Removal**: Calls `registry.remove_pending(&fh, &target)`.
    - If this returns `true`, the lock was found in the pending queue and successfully removed. The function returns `Nlm4CancelRes` with `stat: Nlm4Stats::Granted`.
5.  **Active Lock Check**: If the lock was not pending, the code converts the `PendingLock` representation into an `ActiveLock` representation using `Into`. It then calls `registry.has_active_lock(&fh, &request_as_active)`.
    - If this returns `true`, it means the lock is already held by the client (it was granted between the request and the cancel, or the client was mistaken about it being blocked). The function returns `Nlm4CancelRes` with `stat: Nlm4Stats::Granted`.
6.  **Denial**: If the lock is neither pending nor active, the function returns `Nlm4CancelRes` with `stat: Nlm4Stats::Denied`.

Edge Cases:
- **Invalid Input**: If the arguments contain invalid data (e.g., a caller name exceeding the maximum length), `PendingLock::new` fails, resulting in a `Failed` status rather than a panic.
- **Race Condition (Pending -> Active)**: If a lock transitions from the pending queue to the active list between the time the client sends the request and the server processes it, the logic handles this by checking the active list if the pending check fails. This aligns with RFC 1813, which states that if the lock is not blocked but is held, the server should return `Granted`.

Complexity:
- **Time**:
    - `PendingLock::new`: O(1) (validation checks).
    - `Lock acquisition`: Depends on contention; O(1) in the uncontended case.
    - `remove_pending` / `has_active_lock`: O(N) where N is the number of locks on the specific file (linear scan of the vector).
- **Space**: O(1) additional space (allocates the `target` lock struct).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::service::nlm`**:
    - **`LockRegistry::remove_pending`**: Used to atomically find and remove a lock request from the blocked queue. This is the primary action for a successful cancellation of a blocked request.
    - **`LockRegistry::has_active_lock`**: Used to determine if the lock described in the cancel arguments is currently held by the client. This is necessary to handle the edge case where the lock was granted before the cancel request was processed.
    - **`NlmService::locks`**: The `RwLock` guarding the `LockRegistry`. The module uses `write().await` to gain exclusive access, ensuring that the check-and-remove operation is atomic with respect to other lock operations.
- **From `nfs_mamont::nlm::procedures::cancel`**:
    - **`Cancel` trait**: Defines the asynchronous function signature `cancel` that this module implements.
- **From `nfs_mamont::nlm`**:
    - **`Nlm4Stats`**: Provides the enumeration values (`Granted`, `Denied`, `Failed`) used to signal the result of the operation to the client.

---

## 4. Data Model

Entities:
- No new entities are defined in this module. It operates on existing entities defined in dependencies:
    - `NlmService`
    - `PendingLock`
    - `ActiveLock`
    - `Nlm4CancelArgs`
    - `Nlm4CancelRes`

Relations:
- **`NlmService` implements `Cancel`**: This module provides the implementation logic linking the service's state to the protocol interface.

Global Invariants:
- If `PendingLock::new` returns `Ok`, the resulting lock instance is a valid representation of the arguments provided by the client.

## 5. Error Model

Error Types:
- **`std::io::Error`**: Implicitly returned by `PendingLock::new` (as per the parent module specification) if validation fails (e.g., invalid caller name).

Error Propagation Strategy:
- **Status Code Mapping**: The function does not return a `Result`. Instead, it catches the error from `PendingLock::new` using a `match` statement and maps it to the `Nlm4Stats::Failed` status code within the `Nlm4CancelRes` struct.

Recoverability:
- **Recoverable**: The client receives a `Failed` status, indicating the request could not be processed due to invalid parameters. The client may correct the request and retry, though typically a `CANCEL` failure implies the original lock request is either invalid or already processed.

Panics:
- **Allowed**: No.
- **Conditions**: The code handles the potential error from `PendingLock::new` explicitly. The `write().await` on the `RwLock` is standard async synchronization and does not panic under normal usage (poisoning is handled by the `RwLock` implementation, usually by propagating the poison, but here we assume standard usage).

---

## 6. Traits

List which external traits this module implements:
- **`crate::nlm::procedures::cancel::Cancel`**: Implemented for `NlmService`.
    - `async fn cancel(&self, args: Nlm4CancelArgs) -> Nlm4CancelRes`

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **implement the server-side logic for the NLMv4 `CANCEL` procedure**, enabling clients to retract blocked lock requests or verify the status of locks they believe to be pending. In a distributed locking environment, clients may issue blocking lock requests that enter a wait queue on the server. If the client application times out or the user cancels the operation locally, the client must inform the server to remove the request from the queue to free resources and prevent a "GRANTED" callback from being sent later for a lock that is no longer needed.

The system contains the `NlmService`, which acts as the central authority for lock state. This module connects the abstract `Cancel` trait (defined in the procedures module) to the concrete `LockRegistry` (defined in the parent service module). It ensures that the cancellation logic adheres to the NLM protocol's specific requirements: specifically, that a cancellation is considered successful (`Granted`) if the lock was found in the pending queue and removed, or if the lock is already active (held by the client). It returns `Denied` only if the lock is neither pending nor active.

A typical usage scenario of the system involves a client that previously sent a blocking `LOCK` request. While waiting, the client decides to abort. It sends a `CANCEL` request containing the same lock details. The RPC layer dispatches this to the `cancel` method implemented in this module. The module validates the input, checks the `LockRegistry`, and removes the pending entry if found. It returns a status to the client, confirming the action.

Inside the system, the following things happen and they use this module: The `NlmService` relies on this implementation to maintain the integrity of the pending lock queue. Without this module, the `NlmService` would satisfy the `Nlm` trait requirement (which includes `Cancel`) but would have no logic to actually clean up the queue, leading to resource leaks where abandoned lock requests remain in memory indefinitely, potentially blocking other clients unnecessarily.