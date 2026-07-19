<!-- SPEC_HASH: 8270a53e18d81443e787beb2b579dddf4555ee347de74e0890354586d0919c39 -->
# Module Specification

Module: nfs_mamont::service::nlm::unlock
Rust File: src/service/nlm/unlock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::procedures::unlock`**: Used to import the `Unlock` trait, `Nlm4UnlockArgs`, and `Nlm4UnlockRes`. This module provides the interface definition that this specific module implements, defining the contract for how an unlock operation must behave and the structure of the data involved.
*   **`crate::nlm::Nlm4Stats`**: Used to provide the status constants (`Granted`, `Failed`) that indicate the outcome of the unlock operation to the client.
*   **`super::{check_caller_name, NlmService}`**: Used to access the parent module's utility function `check_caller_name` for input validation and to implement the `Unlock` trait for the `NlmService` struct, which acts as the main service container holding the lock state.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide the concrete implementation of the NLM v4 `UNLOCK` procedure for the `NlmService`. This involves validating the request, atomically modifying the internal lock registry to remove a specific lock, and triggering the granting of any pending locks that may have been blocked by the removed lock.

Inputs:
- `&self`: A reference to the `NlmService` instance, which holds the state (specifically the `locks` registry).
- `args: Nlm4UnlockArgs`: The arguments for the unlock request, containing the transaction `cookie` and the `lock` details (caller name, file handle, offset, length, etc.).

Outputs:
- `Nlm4UnlockRes`: The result structure containing the echoed `cookie` and a `stat` field (`Nlm4Stats`) indicating success (`Granted`) or failure (`Failed`).

Steps:
1.  **Caller Validation**: The `check_caller_name` function is called with the `caller_name` from the arguments. If this check returns an error, the function immediately returns `Nlm4UnlockRes` with `Nlm4Stats::Failed`.
2.  **Lock Acquisition**: The method acquires a write lock on `self.locks` (the internal registry) using `.write().await`. This ensures exclusive access to the lock state for the duration of the operation.
3.  **Lock Removal**: The `remove_by_owner` method is called on the registry with the file handle, caller name, system identifier, offset, and length. If this operation returns an error (e.g., the lock does not exist or an internal error occurs), the function returns `Nlm4Stats::Failed`.
4.  **Pending Lock Granting**: The `grant_pending` method is called on the registry with the file handle. This step attempts to grant any locks that were waiting for the just-removed lock to be released. If this operation fails, the function returns `Nlm4Stats::Failed`.
5.  **Success Response**: If all previous steps succeed, the function returns `Nlm4UnlockRes` with `Nlm4Stats::Granted`.

Edge Cases:
- **Invalid Caller Name**: If the caller name fails validation, the operation fails immediately without modifying the registry.
- **Lock Not Found**: If `remove_by_owner` fails to find the specific lock (or fails for another reason), the operation is considered a failure (`Failed` status is returned).
- **Pending Grant Failure**: If the system successfully removes the lock but fails to process the queue of pending locks for that file, the operation is reported as a failure to the client.

Complexity:
- Time: Depends on the complexity of the internal registry's `remove_by_owner` and `grant_pending` operations, plus the time to acquire the write lock.
- Space: O(1) additional space used within the function, excluding the space held by the registry.

Determinism:
- Deterministic. Given the same state of `self.locks` and the same input arguments, the output will be consistent.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **`Unlock` trait (from `nfs_mamont::nlm::procedures::unlock`)**: Defines the asynchronous `unlock` method signature. This module provides the logic required by this trait, ensuring that the service can respond to NLM unlock requests.
- **`Nlm4UnlockArgs` and `Nlm4UnlockRes` (from `nfs_mamont::nlm::procedures::unlock`)**: These structures carry the data. The module extracts the `cookie` and lock details from `Nlm4UnlockArgs` and constructs the `Nlm4UnlockRes` to return the status.
- **`Nlm4Stats` (from `nfs_mamont::nlm`)**: Provides the enumeration values used to signal the result. Specifically, `Granted` indicates the lock was removed and pending locks processed, while `Failed` covers validation errors, missing locks, or internal registry errors.

---

## 4. Data Model

Entities:
- **`NlmService`**: The service struct implementing the `Unlock` logic. It is assumed to contain a `locks` field (not explicitly listed in the provided facts but used in the code) which acts as the lock registry.
- **`Nlm4UnlockArgs`**: Input entity containing the `cookie` and `Nlm4Lock` details.
- **`Nlm4UnlockRes`**: Output entity containing the `cookie` and `Nlm4Stats`.

Relations:
- **`NlmService` implements `Unlock`**: The service provides the concrete logic for the trait.
- **`NlmService` owns `LockRegistry`**: The service holds the state (accessed via `self.locks`).

Global Invariants:
- **Cookie Echoing**: The `cookie` in the returned `Nlm4UnlockRes` must be identical to the one in `Nlm4UnlockArgs`.
- **Atomicity**: The modification of the lock registry (removal and granting pending) happens under a single write lock, ensuring consistency of the lock state during the operation.

## 5. Error Model

Error Types:
- None explicitly defined in this module. Errors are handled via return values.

Error Propagation Strategy:
- Status Codes. The function does not return a `Result`. Instead, it maps internal errors (validation failure, registry removal failure, pending grant failure) to `Nlm4Stats::Failed`.

Recoverability:
- If `Nlm4Stats::Failed` is returned, the client should infer that the unlock did not succeed or the state is inconsistent. The client may need to retry or investigate the specific lock state, though the protocol does not provide detailed error distinctions here (all paths lead to `Failed`).

Panics:
- Allowed: No (based on visible code).
- Conditions: The code uses `is_err()` checks and returns early. However, a panic could theoretically occur within the `registry` methods (`remove_by_owner`, `grant_pending`) or if `self.locks` is not properly initialized, but these are outside the direct control of this module's logic.

---

## 6. Traits

List which external traits this module implements:
- **`Unlock` (from `crate::nlm::procedures::unlock`)**: Implemented for `NlmService`.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to execute the logic required to release a file lock within the Network Lock Manager (NLM) service. It serves as the bridge between the abstract NLM protocol definition (which defines *what* an unlock looks like) and the concrete lock management state (which defines *how* locks are stored and removed).

The system contains the `NlmService`, which maintains a registry of active locks. When an NFS client finishes working with a file, it sends an unlock request. This module ensures that the request is valid (checking the caller name), atomically removes the specific lock from the registry to prevent race conditions, and immediately checks if any other pending locks can now be granted due to the resource becoming free. This "grant pending" step is crucial for maintaining high concurrency and preventing starvation in the file locking system.

A typical usage scenario involves a client sending an `UNLOCK` RPC. The server routes this to the `NlmService.unlock` method. The method validates the input, locks the internal registry, deletes the lock entry matching the client's request, and attempts to wake up any waiting clients. Finally, it returns a status indicating success or failure.

Inside the system the following things happen and they use the `Nlm4Stats` enum to communicate the outcome (success or generic failure) and the `Nlm4UnlockArgs`/`Nlm4UnlockRes` structures to maintain the protocol contract, ensuring that the transaction ID (cookie) is preserved for the client to match the response. The implementation relies on the assumption that `self.locks` provides a thread-safe, asynchronous interface (`write().await`) for modifying the lock state.