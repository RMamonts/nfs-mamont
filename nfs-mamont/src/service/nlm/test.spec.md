<!-- SPEC_HASH: 5997e5b2f914ad210717ce7101b046c5ad4034e525bbeeb9fc103877164519aa -->
# Module Specification

Module: nfs_mamont::service::nlm::test
Rust File: src/service/nlm/test.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::nlm::procedures::test`**: Used to import the `Test` trait, which defines the asynchronous interface for the NLM TEST procedure, and the data structures `Nlm4TestArgs`, `Nlm4TestRes`, and `Nlm4TestReply` which represent the request and response payloads.
- **`crate::nlm::Nlm4Stats`**: Used to provide the status codes (`Granted`, `Denied`, `Failed`) that indicate the outcome of the test operation in the response.
- **`super::{ActiveLock, NlmService}`**: Used to access the `NlmService` struct (for which the trait is being implemented) and the `ActiveLock` struct, which serves as the internal representation of a lock request used to query the registry.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the `Test` procedure of the NLM protocol for the `NlmService`. This implementation allows a client to query the server to determine if a specific lock request would conflict with existing locks without actually acquiring the lock or modifying the lock state.

Inputs:
- **`&self`**: A reference to the `NlmService` instance, providing access to the internal lock registry.
- **`args: Nlm4TestArgs`**: The arguments of the TEST request, containing the transaction cookie, the exclusive flag, and the lock details (caller name, system ID, file handle, offset, length, opaque handle).

Outputs:
- **`Nlm4TestRes`**: The result structure containing the echoed cookie and the test status. The status indicates whether the lock would be granted, denied (and who holds the conflicting lock), or if the request failed (e.g., invalid arguments).

Steps:
1. **Acquire Read Lock**: The method acquires a read lock on `self.locks` (a `tokio::sync::RwLock<LockRegistry>`) to ensure thread-safe access to the lock state while allowing other concurrent readers.
2. **Construct Lock Request**: It attempts to construct an `ActiveLock` instance from the fields provided in `args.lock` (caller name, system ID, exclusive flag, offset, length, opaque handle).
3. **Validation Handling**: If `ActiveLock::new` returns an `Err` (indicating invalid input, such as an empty or oversized caller name), the method immediately returns `Nlm4TestRes` with `stat` set to `Nlm4Stats::Failed` and `holder` set to `None`.
4. **Conflict Detection**: If the lock request is valid, the method extracts the file handle (`fh`) and calls `registry.find_conflict(&fh, &request)`. This checks the internal registry for any active locks on the specified file that overlap with the requested range and belong to a different owner.
5. **Result Formulation**:
   - If `find_conflict` returns `Some(holder)`, a conflict exists. The method returns `Nlm4TestRes` with `stat` set to `Nlm4Stats::Denied` and `holder` populated with the details of the conflicting lock.
   - If `find_conflict` returns `None`, no conflict exists. The method returns `Nlm4TestRes` with `stat` set to `Nlm4Stats::Granted` and `holder` set to `None`.

Edge Cases:
- **Invalid Arguments**: If the `caller_name` or other fields in `args.lock` fail validation inside `ActiveLock::new`, the operation is aborted with a `Failed` status rather than proceeding to the registry check.
- **Zero-Length Locks**: The logic relies on `ActiveLock` and `find_conflict` to correctly interpret a length of 0 as "lock to end of file".

Complexity:
- **Time**: O(N) where N is the number of active locks on the specific file identified by `args.lock.file_handle`. This is determined by the `find_conflict` method in the registry.
- **Space**: O(1) auxiliary space (excluding the space for the `ActiveLock` instance and the registry itself).

Determinism:
- Deterministic. Given the same state of the lock registry and the same input arguments, the output will always be identical.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::nlm::procedures::test`**:
 - **`Test` Trait**: Defines the contract `async fn test(&self, args: Nlm4TestArgs) -> Nlm4TestRes` which this module implements for `NlmService`.
 - **`Nlm4TestArgs` / `Nlm4TestRes`**: The data structures used to deserialize the RPC request and serialize the response.
- **From `nfs_mamont::service::nlm::mod`**:
 - **`NlmService`**: The struct implementing the service logic. It holds the `RwLock<LockRegistry>` accessed via `self.locks`.
 - **`ActiveLock`**: The internal representation of a lock. Its constructor `new` is used to validate the request parameters before querying the registry.
 - **`LockRegistry::find_conflict`**: The core mechanism used to determine if the requested lock overlaps with any existing locks in the registry.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It implements logic for existing entities defined in dependencies.

Relations:
- **`NlmService` implements `Test`**: This module provides the implementation of the `Test` trait for the `NlmService` struct.

Global Invariants:
- The implementation assumes that `ActiveLock::new` performs necessary validation (e.g., checking `caller_name` length) and returns `Err` if the input is malformed.

## 5. Error Model

Error Types:
- No explicit error types are returned by the `test` function (it returns `Nlm4TestRes`, not a `Result`).

Error Propagation Strategy:
- **Status Codes**: Errors are handled internally and mapped to status codes within the `Nlm4TestRes` structure.
 - Validation failures (e.g., from `ActiveLock::new`) result in `Nlm4Stats::Failed`.
 - Lock conflicts result in `Nlm4Stats::Denied`.

Recoverability:
- **Client-Side**: The client must inspect the `stat` field in the response. A `Failed` status indicates a problem with the request itself, while `Denied` indicates a semantic conflict with the current lock state.

Panics:
- **Allowed**: No.
- **Conditions**: The code handles the `Result` from `ActiveLock::new` explicitly, preventing panics due to invalid input. The `await` on the `RwLock` is standard async behavior and does not introduce panics outside of runtime shutdowns.

---

## 6. Traits

List which external traits this module implements:
- **`crate::nlm::procedures::test::Test`**: Implemented for `NlmService`.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **enable non-blocking lock conflict detection within the Network Lock Manager (NLM) service**. It provides the concrete implementation of the `TEST` procedure, allowing clients to query the server's lock state to determine if a specific lock request would be granted without actually acquiring the lock or modifying the registry.

The system containing this module is the `NlmService`, which manages the state of file locks for an NFS server. The `NlmService` relies on a `LockRegistry` to store active locks. This module connects the high-level RPC interface (defined in `nlm::procedures::test`) with the low-level state management (defined in `service::nlm::mod`). It translates the raw arguments from an RPC request into an internal `ActiveLock` object, validates them, and queries the registry for conflicts.

A typical usage scenario of the system involves an NFS client that wishes to perform an operation but wants to avoid blocking if a lock is not available, or a client that has been denied a lock and wants to know who is holding it. The client sends a `TEST` request. The RPC dispatcher calls the `test` method implemented in this module. The module acquires a read lock on the registry to ensure a consistent view, checks for conflicts using `find_conflict`, and returns a status indicating `Granted` or `Denied` (along with the holder's identity if denied).

Inside the system, this module is crucial for **diagnostics and optimistic locking strategies**. By implementing the `Test` trait, it allows the `NlmService` to fulfill the protocol requirement for conflict polling without exposing the internal complexity of the `LockRegistry` or the `ActiveLock` validation logic to the RPC layer. It ensures that the "read-only" nature of the TEST operation is strictly enforced by using a read lock (`self.locks.read().await`) rather than a write lock, maximizing concurrency for other read operations like tests or shared lock checks.