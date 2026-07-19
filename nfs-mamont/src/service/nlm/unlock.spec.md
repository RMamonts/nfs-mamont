<!-- SPEC_HASH: 8270a53e18d81443e787beb2b579dddf4555ee347de74e0890354586d0919c39 -->
# Module Specification

Module: nfs_mamont::service::nlm::unlock
Rust File: src/service/nlm/unlock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::procedures::unlock`**:
    *   Used to import the `Unlock` trait, `Nlm4UnlockArgs`, and `Nlm4UnlockRes`. This module provides the interface that `NlmService` must implement to handle the NLMv4 UNLOCK procedure, defining the input arguments and the expected response structure.
*   **`crate::nlm::Nlm4Stats`**:
    *   Used to provide the status constants (`Granted`, `Failed`) that are returned in the `Nlm4UnlockRes` to indicate the outcome of the unlock operation to the client.
*   **`super`** (i.e., `crate::service::nlm`):
    *   Used to import `NlmService`, the struct for which the `Unlock` trait is being implemented.
    *   Used to import `check_caller_name`, a helper function to validate the `caller_name` field of the request against protocol constraints.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide the concrete implementation of the `Unlock` trait for `NlmService`. This involves validating the request, atomically modifying the shared lock registry to remove the specified lock, and triggering the processing of any pending lock requests that may now be grantable.

Inputs:
- **`args: Nlm4UnlockArgs`**: The arguments structure passed via the RPC call, containing the transaction `cookie` and the `lock` details (caller name, file handle, system ID, offset, length).

Outputs:
- **`Nlm4UnlockRes`**: The response structure containing the echoed `cookie` and a `stat` field (`Nlm4Stats`) indicating success (`Granted`) or failure (`Failed`).

Steps:
1.  **Validation**: The method calls `check_caller_name` with the `caller_name` from the arguments. If the name is invalid (e.g., empty or too long), the function immediately returns `Nlm4UnlockRes` with `stat: Nlm4Stats::Failed`.
2.  **Lock Acquisition**: The method acquires a write lock on `self.locks` (the `RwLock<LockRegistry>` inside `NlmService`). This ensures exclusive access to the lock state for the duration of the operation.
3.  **Lock Removal**: The method calls `registry.remove_by_owner`, passing the file handle, caller name, system identifier, offset, and length. This removes the specific lock range from the registry. If this operation returns an `Err`, the method returns `Nlm4UnlockRes` with `stat: Nlm4Stats::Failed`.
4.  **Pending Queue Processing**: The method calls `registry.grant_pending` with the file handle. This checks the queue of blocked requests for this file and attempts to grant any that are no longer conflicting. If this operation returns an `Err`, the method returns `Nlm4UnlockRes` with `stat: Nlm4Stats::Failed`.
    *   *Note*: The code contains a `TODO` comment indicating that client notification logic (callbacks) is missing. Consequently, the list of granted locks returned by `grant_pending` is currently ignored.
5.  **Success Response**: If all steps succeed, the method returns `Nlm4UnlockRes` with `stat: Nlm4Stats::Granted`.

Edge Cases:
- **Invalid Caller Name**: Requests with invalid caller names are rejected early without modifying the registry.
- **Registry Errors**: If the underlying `remove_by_owner` or `grant_pending` operations encounter an error (e.g., internal state inconsistency), the operation is aborted, and a `Failed` status is returned.
- **Missing Lock**: The behavior depends on `remove_by_owner`. If the lock does not exist, `remove_by_owner` typically returns `Ok(())` (idempotency), resulting in a `Granted` status, unless an internal error occurs.

Complexity:
- **Time**: O(N + M), where N is the number of active locks on the file (for `remove_by_owner`) and M is the number of pending requests (for `grant_pending`). The complexity is dominated by the `LockRegistry` operations in the parent module.
- **Space**: O(1) additional space on the stack, excluding the space used by the `LockRegistry` itself.

Determinism:
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

-   **From `nfs_mamont::nlm::procedures::unlock`**:
    -   **`Unlock` Trait**: Defines the asynchronous `unlock` method signature that this module implements.
    -   **`Nlm4UnlockArgs` / `Nlm4UnlockRes`**: The data structures used to receive the request and send the response.
-   **From `nfs_mamont::service::nlm` (parent module)**:
    -   **`check_caller_name`**: Used to validate the `caller_name` field before processing.
    -   **`LockRegistry::remove_by_owner`**: Used to perform the actual removal of the lock from the internal state, handling range splitting if necessary.
    -   **`LockRegistry::grant_pending`**: Used to process the queue of waiting locks after a resource is freed.
    -   **`NlmService::locks`**: The `tokio::sync::RwLock` guarding the `LockRegistry`, accessed via `write().await` to ensure mutual exclusion.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on `Nlm4UnlockArgs` and `Nlm4UnlockRes` (defined in `crate::nlm::procedures::unlock`) and mutates the state within `NlmService` (defined in `crate::service::nlm`).

Relations:
- **Implementation**: `NlmService` implements the `Unlock` trait.

Global Invariants:
- The `cookie` in the returned `Nlm4UnlockRes` must match the `cookie` in the input `Nlm4UnlockArgs`. The code ensures this by copying `args.cookie` into the return struct in all branches.

## 5. Error Model

Error Types:
- **`std::io::Error`**: Implicitly used via `check_caller_name` and potentially returned by `LockRegistry` methods.

Error Propagation Strategy:
- The method does not return a Rust `Result`. Instead, errors are caught and mapped to the `Nlm4Stats::Failed` variant within the `Nlm4UnlockRes` struct. This aligns with the NLM protocol, where status is communicated via the message body rather than transport-level errors.

Recoverability:
- Recoverable by the client. The client receives a `Failed` status and may choose to retry or log the error.

Panics:
- **Allowed**: No.
- **Conditions**: The code does not contain explicit `panic!` or `unwrap!` calls. It relies on `is_err()` checks and early returns.

---

## 6. Traits

List which external traits this module implements:
- **`crate::nlm::procedures::unlock::Unlock`**: Implemented for `NlmService`.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **implement the server-side logic for the NLMv4 UNLOCK procedure**, effectively bridging the gap between the abstract RPC protocol definition and the concrete in-memory lock state managed by `NlmService`. It is the mechanism by which the server releases resources held by a client, allowing other clients to acquire locks on the file.

The system contains a distributed lock manager where locks are finite resources that must be explicitly released. This module is the specific handler for the "release" action. It ensures that when a client requests an unlock, the request is valid (caller name check), the operation is performed atomically against the shared registry (using the write lock), and the side effects of releasing a lock (potentially unblocking waiting clients) are triggered immediately.

A typical usage scenario of the system involves a client closing a file or explicitly calling an unlock function. The RPC layer deserializes the request into `Nlm4UnlockArgs` and invokes the `unlock` method on `NlmService`. Inside the system, the following things happen and they use this module: The implementation validates the input, then accesses the `LockRegistry` to remove the specific byte-range lock identified by the file handle and owner details. Once removed, it calls `grant_pending` to see if any other clients were waiting for this lock. While the code currently identifies which locks can be granted, it contains a `TODO` indicating that the actual notification (callback) to those clients is not yet implemented. Finally, it returns a `Granted` status to the caller, confirming the release. Without this module, the `NlmService` would be unable to process unlock requests, leading to permanent locks and resource starvation.