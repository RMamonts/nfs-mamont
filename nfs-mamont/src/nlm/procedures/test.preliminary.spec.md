<!-- SPEC_HASH: eefbc4169d4d0ae441cbc9a0e70e66fe287cb3026ec41ca76642d6e82da04fa0 -->
# Module Specification

Module: nfs_mamont::nlm::procedures::test
Rust File: src/nlm/procedures/test.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::cookie::Cookie`**:
    *   Used to identify the transaction. The `cookie` is passed in the arguments and must be returned in the result to match the asynchronous request with the response in the NLM protocol.
*   **`crate::nlm::holder::Nlm4Holder`**:
    *   Used to describe the owner of a conflicting lock. If a lock request is tested and found to be blocked, this structure provides details about the process holding the lock (system ID, offset, length, etc.).
*   **`crate::nlm::lock::Nlm4Lock`**:
    *   Used to specify the parameters of the lock being tested. It includes the file handle, caller name, and the range (offset/length) to check.
*   **`crate::nlm::Nlm4Stats`**:
    *   Used to report the status of the test operation. It indicates whether the lock is granted, denied, or if an error occurred (e.g., stale file handle).
*   **`trait_variant`**:
    *   Used via the `#[trait_variant::make(Send)]` attribute to transform the async `Test` trait into an object-safe trait that can be used as a `dyn Trait` while ensuring it is `Send`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the data structures and the asynchronous interface required to implement the NLMv4 `TEST` procedure. This procedure allows a client to poll the server to determine if a lock request would be granted without actually acquiring the lock.

Inputs:
- `args: Nlm4TestArgs`: A structure containing the `cookie` (transaction ID), a boolean `exclusive` flag indicating the lock type, and a `lock` structure (`Nlm4Lock`) describing the file and region to test.

Outputs:
- `Nlm4TestRes`: A structure containing the `cookie` (echoed from the request) and a `test_stat` field (`Nlm4TestReply`) containing the result status and optional holder information.

Steps:
1.  **Request Reception**: The server receives an NLM `TEST` request, deserialized into `Nlm4TestArgs`.
2.  **Lock Check**: The implementation of the `Test` trait inspects the `Nlm4Lock` details against the current lock manager state.
3.  **Result Construction**:
    - If the lock can be granted, the implementation returns `Nlm4TestRes` with `stat` set to `Nlm4Stats::Granted` and `holder` set to `None`.
    - If the lock cannot be granted, it returns `Nlm4TestRes` with `stat` set to `Nlm4Stats::Denied` (or another appropriate error code) and `holder` set to `Some(Nlm4Holder)` describing the conflicting lock.
4.  **Response Transmission**: The `Nlm4TestRes` is serialized and sent back to the client.

Edge Cases:
- **Status Mismatch**: The `holder` field in `Nlm4TestReply` is semantically valid only when the lock is denied. While the type system allows `Some(holder)` for any status, the protocol dictates it should be populated only on conflict.
- **Zero Length Locks**: The `Nlm4Lock` may contain a length of 0, which typically means "to end of file". The implementation must handle this range correctly.

Complexity:
- Time: O(1) for data structure access. The complexity of the actual lock check depends on the implementation of the `Test` trait.
- Space: O(1) for the structures themselves (fixed size fields, though `String` and `Vec` inside dependencies imply heap allocation).

Determinism:
- Deterministic (Data structures define the layout; the trait defines the contract).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **`nfs_mamont::nlm::cookie`**:
    - `Cookie`: A newtype wrapper around `u64` used to match requests to responses. It provides `new`, `raw`, and `is_zero` methods.
- **`nfs_mamont::nlm::holder`**:
    - `Nlm4Holder`: A structure containing `exclusive` (bool), `system_identifier` (i32), `opaque_handle`, `lock_offset` (u64), and `lock_length` (u64). It describes the entity holding a lock.
- **`nfs_mamont::nlm::lock`**:
    - `Nlm4Lock`: A structure containing `caller_name` (String), `file_handle`, `opaque_handle`, `system_identifier` (i32), `lock_offset` (u64), and `lock_length` (u64). It describes the requested lock region.
- **`nfs_mamont::nlm`**:
    - `Nlm4Stats`: An enum representing the status of the NLM operation (e.g., `Granted`, `Denied`, `DeniedNolocks`, `StaleFh`).

---

## 4. Data Model

Entities:
- `Nlm4TestArgs`: Arguments for the TEST procedure.
- `Nlm4TestRes`: Result of the TEST procedure.
- `Nlm4TestReply`: The specific status payload within the result.

Relations:
- `Nlm4TestArgs` **contains** `Cookie` (1:1).
- `Nlm4TestArgs` **contains** `Nlm4Lock` (1:1).
- `Nlm4TestRes` **contains** `Cookie` (1:1).
- `Nlm4TestRes` **contains** `Nlm4TestReply` (1:1).
- `Nlm4TestReply` **contains** `Nlm4Stats` (1:1).
- `Nlm4TestReply` **optionally contains** `Nlm4Holder` (0:1).

Global Invariants:
- The `cookie` in `Nlm4TestRes` must be identical to the `cookie` in `Nlm4TestArgs` for the client to correctly associate the response with the request.
- The `holder` field in `Nlm4TestReply` should only be present if the `stat` field indicates a conflict (e.g., `Denied`).

---

## 5. Error Model

Error Types:
- None defined explicitly in this module. Error conditions are communicated via the `Nlm4Stats` enum within the `Nlm4TestReply` structure.

Error Propagation Strategy:
- Embedded Status. The `test` method returns `Nlm4TestRes` directly, not a `Result`. Success or failure is determined by inspecting the `stat` field of the reply.

Recoverability:
- Dependent on the `Nlm4Stats` value. For example, `Denied` implies the client may retry later, while `StaleFh` implies a permanent error with the file handle.

Panics:
- Allowed: No.
- Conditions: The data structures themselves do not perform operations that can panic. The trait implementation might panic, but the interface definition does not specify it.

---

## 6. Traits

List which external traits this module implements:
- None explicitly implemented in the provided source code. The module defines the `Test` trait but does not implement external traits on its structs.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to implement the non-destructive lock checking capability of the Network Lock Manager (NLM) version 4 protocol. In distributed file systems, clients often need to know if a lock is available before attempting a blocking operation or to implement "try-lock" semantics without modifying the server's lock state. The `TEST` procedure serves this specific purpose.

This system contains the data contracts (`Nlm4TestArgs`, `Nlm4TestRes`) and the asynchronous service definition (`Test` trait) required to handle these requests. It relies on the `Cookie` type to ensure transactional integrity across the network, `Nlm4Lock` to define the scope of the query, and `Nlm4Holder` to provide feedback on conflicts. By separating the test logic into a distinct trait and data structures, the system allows the NLM service to handle lock polling independently from lock acquisition (`LOCK`) or release (`UNLOCK`).

A typical usage scenario of the system involves a client application attempting to access a file region. To avoid blocking indefinitely, the client issues a `TEST` call. The server receives the `Nlm4TestArgs`, checks its internal lock state (implementation defined by the `Test` trait), and returns `Nlm4TestRes`. If the result is `Denied`, the `Nlm4Holder` information tells the client exactly who holds the lock (offset, length, owner), allowing the client to decide whether to wait, cancel, or notify the user.

Inside the system the following things happen and they use the `Test` trait to abstract the logic of determining lock availability. The trait is marked `Send` to ensure it can be used in an asynchronous, multi-threaded server environment (like `tokio` or `async-std`), where the lock check might involve querying shared state or a database. The `Nlm4Stats` enum provides a standardized way to encode success or various failure modes (like `StaleFh` or `Deadlock`) directly in the response structure, avoiding the need for separate exception handling paths in the RPC layer.