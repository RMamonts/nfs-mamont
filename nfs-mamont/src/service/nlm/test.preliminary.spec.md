<!-- SPEC_HASH: 5997e5b2f914ad210717ce7101b046c5ad4034e525bbeeb9fc103877164519aa -->
# Module Specification

Module: nfs_mamont::service::nlm::test
Rust File: src/service/nlm/test.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

* **`crate::nlm::procedures::test::{Nlm4TestArgs, Nlm4TestReply, Nlm4TestRes, Test}`**:
    * Used to define the interface contract. `Test` is the asynchronous trait implemented by this module. `Nlm4TestArgs` and `Nlm4TestRes` are the input and output data structures required by the NLMv4 protocol.
* **`crate::nlm::Nlm4Stats`**:
    * Used to provide status codes indicating the result of the test operation (e.g., `Granted`, `Denied`, `Failed`).
* **`super::{ActiveLock, NlmService}`**:
    * `NlmService` is the struct that owns the lock registry and implements the business logic.
    * `ActiveLock` is a helper type used to normalize and validate the lock parameters (offset, length, caller) before querying the registry.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the `Test` trait for `NlmService`, enabling the server to answer non-blocking queries regarding lock availability. This allows clients to check if a lock request would conflict with existing locks without actually acquiring the lock.

Inputs:
- `args: Nlm4TestArgs`: Contains the `cookie` for transaction tracking, an `exclusive` boolean flag, and a `lock` structure (`Nlm4Lock`) with file handle, caller name, and range details.

Outputs:
- `Nlm4TestRes`: Contains the echoed `cookie` and a `test_stat` (`Nlm4TestReply`). The reply includes a status code (`Nlm4Stats`) and, if denied, information about the conflicting lock holder.

Steps:
1. **Acquire Read Lock**: The method acquires a read lock on `self.locks` (an internal registry) to ensure thread-safe access to the current lock state without blocking other readers.
2. **Normalize Request**: It attempts to construct an `ActiveLock` instance from the arguments. This step validates the lock parameters (caller name, system ID, offset, length, etc.).
3. **Validation Failure Handling**: If `ActiveLock::new` returns an `Err`, the method immediately returns `Nlm4TestRes` with `stat` set to `Nlm4Stats::Failed` and no holder information.
4. **Conflict Detection**: If validation succeeds, the method calls `registry.find_conflict(&fh, &request)`, passing the file handle and the normalized `ActiveLock`.
5. **Result Formulation**:
    - If `find_conflict` returns `Some(holder)`, the lock is denied. The response is constructed with `Nlm4Stats::Denied` and the holder details.
    - If `find_conflict` returns `None`, the lock is available. The response is constructed with `Nlm4Stats::Granted`.

Edge Cases:
- **Invalid Lock Parameters**: If the offset, length, or other fields in `Nlm4Lock` are invalid (causing `ActiveLock::new` to fail), the operation returns `Failed` rather than checking for conflicts.
- **Zero-Length Locks**: The behavior depends on the implementation of `ActiveLock` and `find_conflict`, but typically a length of 0 implies locking to the end of the file.

Complexity:
- Time: O(1) or O(N) depending on the implementation of `find_conflict` in the registry (not visible here), plus the overhead of acquiring the read lock.
- Space: O(1) for the `ActiveLock` creation and result structures.

Determinism:
- Deterministic. Given the same state of `self.locks` and the same input arguments, the output will be identical.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **`nfs_mamont::nlm::procedures::test`**:
    - `Test` trait: Defines the `async fn test(&self, args: Nlm4TestArgs) -> Nlm4TestRes` signature that this module implements.
    - `Nlm4TestArgs`: Provides the `cookie`, `exclusive` flag, and `Nlm4Lock` (containing `caller_name`, `file_handle`, `lock_offset`, `lock_length`, etc.).
    - `Nlm4TestRes`: The wrapper for the response containing the `cookie` and `Nlm4TestReply`.
    - `Nlm4TestReply`: Contains the `stat` (`Nlm4Stats`) and optional `holder` (`Nlm4Holder`).
- **`nfs_mamont::nlm`**:
    - `Nlm4Stats`: Enum used to set the status in the response. Specifically `Granted`, `Denied`, and `Failed` are used here.
- **`nfs_mamont::service::nlm`**:
    - `NlmService`: The struct implementing the logic. It is assumed to contain a `locks` field that acts as a registry for active locks.

---

## 4. Data Model

Entities:
- `NlmService`: The service context holding the lock registry.
- `ActiveLock`: (Internal) A validated representation of a lock request used for comparison.

Relations:
- `NlmService` **manages** a collection of locks (accessed via `self.locks`).
- `Nlm4TestArgs` **is converted to** `ActiveLock` for internal processing.

Global Invariants:
- The `cookie` in the response must match the `cookie` in the request.
- The `holder` field is populated only if the status is `Denied`.

---

## 5. Error Model

Error Types:
- **Input Validation Error**: Represented by `Nlm4Stats::Failed`. This occurs when `ActiveLock::new` returns an `Err`, indicating invalid lock parameters (e.g., overflow, invalid ranges).

Error Propagation Strategy:
- Embedded Status. The method does not return a `Result` type. Instead, errors are encoded within the `Nlm4TestRes` structure via the `stat` field.

Recoverability:
- `Nlm4Stats::Failed`: Indicates a client error (bad arguments). The client should correct the request before retrying.
- `Nlm4Stats::Denied`: Indicates a resource conflict. The client may retry later or wait for a notification.

Panics:
- Allowed: No.
- Conditions: The code performs error checking on `ActiveLock::new` and handles the registry access safely. Panics should not occur under normal operation.

---

## 6. Traits

List which external traits this module implements:
- **`crate::nlm::procedures::test::Test`**: Implemented for `NlmService`.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to provide the concrete server-side logic for the NLM (Network Lock Manager) TEST procedure. The NLM protocol requires a mechanism for clients to poll the server to determine if a lock request would be granted without actually acquiring the lock or modifying the server's state. This is essential for implementing "try-lock" semantics or avoiding indefinite blocking in distributed file systems.

The system consists of the `NlmService`, which acts as the authoritative state manager for file locks, and this module, which bridges the abstract protocol definition (the `Test` trait) with the concrete lock management logic. It relies on `ActiveLock` to ensure that incoming requests are semantically valid before they are checked against the registry. The registry itself (accessed via `self.locks`) is assumed to handle the complexity of range intersection and lock compatibility.

A typical usage scenario involves a client needing to access a file but wanting to avoid blocking if the file is already locked. The client sends a `TEST` request. The `NlmService` receives this request, acquires a read lock on its internal registry to ensure a consistent view, and validates the request parameters. If valid, it queries the registry for conflicts. If a conflict exists, the system returns `Denied` along with the details of the lock holder (system ID, offset, etc.), allowing the client to decide its next action. If no conflict exists, it returns `Granted`.

Inside the system, the following things happen: the module decouples the RPC handling (defined in `nlm::procedures::test`) from the state management (defined in `service::nlm`). By implementing the `Test` trait, `NlmService` integrates seamlessly into the larger RPC framework. The use of a read lock (`self.locks.read().await`) implies that the system is designed for high concurrency, allowing multiple TEST requests to be processed simultaneously as long as no write operation (like LOCK or UNLOCK) is in progress.

**Assumptions:**
- The type of `self.locks` is not fully specified in the provided context but is inferred to be a thread-safe, lockable container (likely a `RwLock` wrapping a registry type) that exposes a `find_conflict` method.
- The `ActiveLock::new` function performs necessary validation (e.g., checking for integer overflows in offset/length) and returns a `Result` indicating success or failure.