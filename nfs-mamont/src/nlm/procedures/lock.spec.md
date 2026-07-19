<!-- SPEC_HASH: 4f314d0c6ce200e632bd69459522874ee4a5a18251701135fa2166dd2f006046 -->
# Module Specification

Module: nfs_mamont::nlm::procedures::lock
Rust File: src/nlm/procedures/lock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::nlm::cookie`**: Used to provide the `Cookie` type, which serves as a transaction identifier in both the request (`Nlm4LockArgs`) and the response (`Nlm4LockRes`) to allow the client to match asynchronous RPC replies.
- **`crate::nlm::lock`**: Used to provide the `Nlm4Lock` type, which encapsulates the specific details of the lock request (caller name, file handle, owner, offset, length) within the `Nlm4LockArgs` structure.
- **`crate::nlm`**: Used to provide the `Nlm4Stats` enum, which defines the standardized status codes (e.g., `Granted`, `Denied`) returned in the `Nlm4LockRes` structure.
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` attribute macro to transform the async `Lock` trait into an object-safe trait that also implements `Send`, allowing it to be used as a trait object in a multi-threaded asynchronous context (e.g., passed to an RPC dispatcher).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the data structures and the asynchronous interface required to handle the NLMv4 `LOCK` procedure. This module acts as the contract layer, defining the shape of inputs and outputs for lock acquisition, delegating the actual concurrency control and state management to implementations of the `Lock` trait.

Inputs:
- **`Nlm4LockArgs`**: A structure containing the parameters for a lock request.
    - `cookie`: Transaction identifier.
    - `block`: Boolean indicating if the call should block until the lock is available.
    - `exclusive`: Boolean indicating if the lock is exclusive (write) or shared (read).
    - `lock`: `Nlm4Lock` structure containing file handle, owner, and range.
    - `reclaim`: Boolean indicating if the lock is being reclaimed after a server restart.
    - `state`: The state value from the local NSM (Network Status Monitor).
- **`Lock` trait implementation**: A reference to an object implementing the `Lock` trait.

Outputs:
- **`Nlm4LockRes`**: A structure containing the result of the lock attempt.
    - `cookie`: The transaction identifier echoed back to the client.
    - `stat`: A `Nlm4Stats` enum variant indicating the outcome (e.g., `Granted`, `Denied`, `Blocked`).

Steps:
1. The RPC layer deserializes an incoming NLMv4 `LOCK` request into a `Nlm4LockArgs` struct.
2. The server invokes the `lock` method on an object implementing the `Lock` trait, passing the `Nlm4LockArgs`.
3. The implementation of `lock` determines if the lock can be granted based on the `block`, `exclusive`, and `reclaim` flags, as well as the current lock state (managed externally).
4. The implementation returns a `Nlm4LockRes`.
5. The RPC layer serializes the `Nlm4LockRes` and sends it back to the client.

Edge Cases:
- **Blocking Behavior**: If `args.block` is `true`, the implementation of `lock` is expected to suspend the async task until the lock is available, potentially returning `Nlm4Stats::Blocked` if it queues the request for a callback (though the immediate return type is `Nlm4LockRes`, implying a direct response or a queued status).
- **Reclaim**: If `args.reclaim` is `true`, the implementation should typically grant the lock without checking for conflicts, as it is part of a crash recovery protocol.
- **Zero Cookie**: While `Cookie` is a wrapper for `u64`, the protocol semantics usually imply that the cookie in the response must match the cookie in the request.

Complexity:
- **Time**: O(1) for struct creation/access. The complexity of the `lock` method execution depends entirely on the external implementation (e.g., lock manager state lookup).
- **Space**: O(1) for the structs themselves, excluding the size of internal strings/vectors within `Nlm4Lock`.

Determinism:
- **Deterministic** (Struct definitions). The execution of the `lock` method is non-deterministic depending on the external state of the lock manager and network timing.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::nlm::lock`**:
    - **`Nlm4Lock`**: This structure is embedded in `Nlm4LockArgs`. It carries the validated `caller_name` and `file_handle`. The validation logic (checking `caller_name` length) occurs when `Nlm4Lock` is constructed, ensuring that `Nlm4LockArgs` cannot exist with an invalid caller name if constructed correctly.
- **From `crate::nlm::cookie`**:
    - **`Cookie`**: Used to correlate requests and responses. It provides type safety for the transaction ID, preventing it from being confused with other `u64` fields.
- **From `crate::nlm`**:
    - **`Nlm4Stats`**: Provides the vocabulary for the result status. The `Lock` trait implementation must select the appropriate variant (e.g., `Denied` vs `Granted`) based on the logic of the lock operation.

---

## 4. Data Model

Entities:
- **`Nlm4LockArgs`**: The argument structure for the LOCK procedure. It aggregates the transaction context (`cookie`), the lock mode (`exclusive`), the blocking policy (`block`), recovery status (`reclaim`), NSM state (`state`), and the specific lock target (`lock`).
- **`Nlm4LockRes`**: The result structure for the LOCK procedure. It aggregates the transaction context (`cookie`) and the operation status (`stat`).
- **`Lock`**: An asynchronous trait defining the behavior of a lock handler.

Relations:
- **Composition**: `Nlm4LockArgs` contains one `Cookie` and one `Nlm4Lock`.
- **Composition**: `Nlm4LockRes` contains one `Cookie` and one `Nlm4Stats`.
- **Dependency**: The `Lock` trait uses `Nlm4LockArgs` as input and `Nlm4LockRes` as output.

Global Invariants:
- The `cookie` field in `Nlm4LockRes` returned by an implementation of `Lock` must be identical to the `cookie` field in the `Nlm4LockArgs` provided to the call, to ensure the client can match the response to the correct request context.

## 5. Error Model

Error Types:
- None defined in this module.

Error Propagation Strategy:
- The `Lock::lock` method does not return a `Result`. Instead, protocol-level errors or failures to acquire a lock are communicated via the `stat` field of `Nlm4LockRes` using variants like `Nlm4Stats::Denied`, `Nlm4Stats::Failed`, or `Nlm4Stats::DeniedNolocks`. System-level panics or Rust errors in the implementation are considered fatal bugs, not expected protocol outcomes.

Recoverability:
- Recoverable at the protocol level. A client receiving a `Denied` status can retry or abort based on the `block` flag and its own logic.

Panics:
- Allowed: No (in the provided code).
- Conditions: The structs and trait definition themselves do not panic. However, an implementation of the `Lock` trait might panic if it encounters internal inconsistencies (e.g., poisoned mutexes), but this is not enforced by the module.

---

## 6. Traits

List which external traits this module implements:
- None.

Traits defined by this module:
- **`Lock`**: An asynchronous trait with a single method `lock`. It is marked `Send` via `trait_variant`, allowing it to be used as a generic constraint or trait object across threads.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the specific interface for the `LOCK` procedure of the Network Lock Manager version 4 (NLMv4) protocol. The system contains an NFS server that needs to support file locking to ensure data integrity when multiple clients access shared files. The NLM protocol runs as a separate auxiliary service to handle these locks. This module provides the necessary Rust types (`Nlm4LockArgs`, `Nlm4LockRes`) and the asynchronous trait (`Lock`) that act as the boundary between the generic RPC transport layer and the specific lock management logic.

A typical usage scenario of the system involves a client sending an NLM `LOCK` request over the network. The RPC layer deserializes the request into `Nlm4LockArgs`. It then invokes the `lock` method on a service object implementing this module's `Lock` trait. The service object checks the `block` flag: if true, it may wait for the lock; if false, it returns immediately. It also checks the `reclaim` flag to handle server crash recovery gracefully. The result is returned as `Nlm4LockRes`, containing a status code from `Nlm4Stats`.

Inside the system, the following things happen and they use this module:
1. **Type Safety and Validation**: By using `Nlm4Lock` (from the dependency) inside `Nlm4LockArgs`, the system ensures that the lock request includes a validated caller name and file handle before it reaches the lock handler.
2. **Async Dispatch**: The `Lock` trait, generated with `Send` capabilities, allows the lock handler to perform asynchronous operations (like waiting on a semaphore or updating a distributed state store) without blocking the main RPC thread pool.
3. **Protocol Compliance**: The module explicitly includes fields like `state` (NSM state) and `reclaim`, which are required by RFC 1813 for robust crash recovery. This ensures that the server can distinguish between new lock requests and attempts by clients to restore their state after a failure.

Without this module, the RPC layer would lack a structured way to pass lock arguments to the business logic, and the specific semantics of the NLMv4 `LOCK` operation (such as blocking vs. non-blocking behavior) would be undefined or implemented ad-hoc, leading to potential protocol violations.