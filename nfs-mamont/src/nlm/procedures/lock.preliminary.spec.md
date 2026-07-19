<!-- SPEC_HASH: 4f314d0c6ce200e632bd69459522874ee4a5a18251701135fa2166dd2f006046 -->
# Module Specification

Module: nfs_mamont::nlm::procedures::lock
Rust File: src/nlm/procedures/lock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::cookie::Cookie`**:
    *   Used as a transaction identifier in both `Nlm4LockArgs` and `Nlm4LockRes` to match asynchronous procedure calls with their responses.
*   **`crate::nlm::lock::Nlm4Lock`**:
    *   Used within `Nlm4LockArgs` to encapsulate the specific details of the lock request, including the file handle, owner identification, and the byte range (offset/length) to be locked.
*   **`crate::nlm::Nlm4Stats`**:
    *   Used within `Nlm4LockRes` to provide a standardized status code indicating the outcome of the lock operation (e.g., `Granted`, `Denied`, `Blocked`).
*   **`trait_variant`**:
    *   Used via the `#[trait_variant::make(Send)]` attribute on the `Lock` trait. This macro generates an object-safe version of the async trait that implements `Send`, allowing the trait to be used as a trait object (e.g., `dyn Lock`) in multi-threaded contexts.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in the module.
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface and data structures required to handle the NLMv4 `LOCK` procedure. This module abstracts the raw protocol arguments into structured Rust types and defines an asynchronous contract that implementations must fulfill to manage file locks.

Inputs:
- `args: Nlm4LockArgs`: A structure containing the lock request parameters.
    - `cookie`: Transaction ID.
    - `block`: Boolean indicating if the call should block.
    - `exclusive`: Boolean indicating lock type (exclusive vs shared).
    - `lock`: Detailed lock information (`Nlm4Lock`).
    - `reclaim`: Boolean indicating if this is a recovery operation.
    - `state`: The state value from the local NSM.

Outputs:
- `Nlm4LockRes`: A structure containing the result of the lock operation.
    - `cookie`: The transaction ID mirroring the request.
    - `stat`: The status code (`Nlm4Stats`).

Steps:
1.  **Request Reception**: An external entity (e.g., an RPC handler) constructs `Nlm4LockArgs` from incoming network data.
2.  **Dispatch**: The `lock` method of an implementation of the `Lock` trait is invoked with these arguments.
3.  **Processing (Implementation Defined)**:
    - The implementation inspects the `block` flag. If `true`, the asynchronous function awaits until the lock is available or an error occurs. If `false`, it checks availability immediately.
    - The implementation inspects the `reclaim` flag. If `true`, it attempts to re-establish a lock held prior to a server restart, typically ignoring conflicts during a grace period.
    - The implementation checks the `Nlm4Lock` details (file handle, offset, length) against the current lock state.
4.  **Response Construction**: The implementation returns `Nlm4LockRes`.
    - If successful, `stat` is set to `Granted`.
    - If blocked and `block` is `true`, `stat` is set to `Blocked` (and the actual grant is communicated later via a callback, though this module only defines the immediate return).
    - If denied, `stat` is set to `Denied` or a specific error code.

Edge Cases:
- **Reclaim State**: The `reclaim` flag modifies the logic to allow restoring locks without immediate denial, usually valid only during a server's grace period after a crash.
- **Blocking Behavior**: The `block` flag dictates whether the `async fn` should yield or return immediately. This maps directly to the NLM protocol's blocking vs non-blocking modes.
- **Zero Length**: While not explicitly validated in this module, the `Nlm4Lock` dependency implies a `lock_length` field where `0` conventionally means "to end of file".

Complexity:
- Time: Unbounded. Depends on the implementation of the `Lock` trait and the `block` flag. If `block` is true, it may wait indefinitely.
- Space: O(1) for the data structures themselves (excluding the underlying storage for the strings/handles they contain).

Determinism:
- Non-deterministic. The result depends on the external state of the lock manager and the timing of other clients.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::nlm::cookie`**:
    - **`Cookie`**: Acts as a correlation ID. The `raw` method is likely used by the RPC layer to serialize the ID, while `new` is used to deserialize it. The `is_zero` method is likely not used here but is part of the type's general utility.
- **From `nfs_mamont::nlm::lock` (Assumptions based on facts)**:
    - **`Nlm4Lock`**: Provides the `vfs::file::Handle` which links the NLM lock to the actual file system entity. It also provides `lock_offset` and `lock_length` defining the range. The `caller_name` and `system_identifier` are used to identify the owner of the lock for conflict detection.
- **From `nfs_mamont::nlm` (Assumptions based on facts)**:
    - **`Nlm4Stats`**: Provides the vocabulary for the response. The `Lock` implementation must select the correct variant (e.g., `Blocked` vs `Denied`) based on the logic flow.

---

## 4. Data Model

Entities:
- **`Nlm4LockArgs`**: A Data Transfer Object (DTO) representing a lock request. It aggregates the transaction context (`cookie`), the lock semantics (`block`, `exclusive`, `reclaim`), the lock target (`lock`), and the NSM state (`state`).
- **`Nlm4LockRes`**: A Data Transfer Object (DTO) representing a lock response. It couples the transaction context (`cookie`) with the operation result (`stat`).

Relations:
- `Nlm4LockArgs` *contains* `Cookie` (1:1).
- `Nlm4LockArgs` *contains* `Nlm4Lock` (1:1).
- `Nlm4LockRes` *contains* `Cookie` (1:1).
- `Nlm4LockRes` *contains* `Nlm4Stats` (1:1).

Global Invariants:
- The `cookie` field in `Nlm4LockRes` returned by the `Lock` trait must be identical to the `cookie` field in the `Nlm4LockArgs` received by the trait method. This is a strict requirement of the NLM protocol for request-response matching.

---

## 5. Error Model

Error Types:
- None defined in this module. The module does not use Rust's `Result<T, E>` for the primary return type of the trait.

Error Propagation Strategy:
- **Status Codes**: Errors are propagated via the `stat` field of `Nlm4LockRes` using the `Nlm4Stats` enum.
    - Example: `Denied` if a conflict exists.
    - Example: `DeniedGracePeriod` if the server is not ready to accept non-reclaim locks.
    - Example: `StaleFh` if the file handle in `Nlm4Lock` is invalid.

Recoverability:
- Recoverability is determined by the specific `Nlm4Stats` variant returned. For instance, `Denied` suggests the client might retry later, whereas `Failed` suggests a permanent issue.

Panics:
- Allowed: No.
- Conditions: The code consists solely of struct definitions and a trait definition. There is no executable logic in this module that could panic.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines a trait (`Lock`) but does not implement external traits on its types (other than standard derives implied by struct definitions, though they are not visible in the snippet, they are standard practice).

Traits defined by this module:
- **`Lock`**: An asynchronous trait (made `Send` via macro) defining the contract for handling NLMv4 lock requests.

---

## 7. Overview

This section is needed for the evolution of project understanding.
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module.
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed.
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module.
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level.
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to implement the server-side logic for the Network Lock Manager (NLM) version 4 protocol, specifically the `LOCK` procedure. The NLM protocol is a critical component in NFS environments that allows clients to coordinate access to files, preventing data corruption when multiple clients attempt to write to the same resource.

This system contains the data structures and the asynchronous interface required to decouple the network protocol handling (serialization/deserialization) from the actual lock management logic (state tracking, conflict resolution). By defining `Nlm4LockArgs` and `Nlm4LockRes`, the module provides a strongly-typed representation of the RFC 1813 specification. The `Lock` trait serves as the boundary where the generic RPC machinery hands off control to the specific lock manager implementation.

A typical usage scenario of the system involves an NFS server receiving an RPC request for a lock operation. The RPC layer deserializes the payload into `Nlm4LockArgs`. It then invokes the `lock` method on an object implementing the `Lock` trait (e.g., a `LockManager` service). This implementation checks the `Nlm4Lock` details against the VFS (`vfs::file::Handle`) and internal state. If the `block` flag is set and a conflict exists, the implementation suspends the task (using `async` await) until the lock is free, eventually returning `Nlm4LockRes` with a `Granted` status. If `block` is false, it returns immediately with `Denied`.

Inside the system the following things happen and they use the `Cookie` type to ensure that the response is correctly associated with the specific request, and the `Nlm4Stats` enum to communicate the outcome precisely according to the protocol standard, handling complex scenarios like crash recovery (`reclaim` flag) and grace periods.