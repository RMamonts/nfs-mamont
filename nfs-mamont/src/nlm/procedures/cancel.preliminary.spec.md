<!-- SPEC_HASH: 1d76061f01f84c3ce74c405f3802c8020196cd77b37c4b32cd19fe0ad44d0c11 -->
# Module Specification

Module: nfs_mamont::nlm::procedures::cancel
Rust File: src/nlm/procedures/cancel.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::cookie::Cookie`**:
    *   Used as a transaction identifier within `Nlm4CancelArgs` and `Nlm4CancelRes` to match the CANCEL request with the original LOCK request and its corresponding response.
*   **`crate::nlm::lock::Nlm4Lock`**:
    *   Used within `Nlm4CancelArgs` to specify the precise lock parameters (file handle, offset, length, owner) that identify the pending lock request to be canceled.
*   **`crate::nlm::Nlm4Stats`**:
    *   Used within `Nlm4CancelRes` to indicate the success or failure of the cancellation operation (e.g., `Granted` if canceled, `Denied` if no matching blocked lock was found).
*   **`trait_variant`**:
    *   Used via the `#[trait_variant::make(Send)]` attribute macro on the `Cancel` trait. This allows the asynchronous trait to be converted into a trait object that is `Send`, enabling dynamic dispatch across threads in an asynchronous runtime context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module.
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the data structures and the asynchronous service interface required to implement the NLMv4 `CANCEL` procedure. This procedure allows a client to withdraw a previously issued, blocked lock request.

Inputs:
- `Nlm4CancelArgs`: A structure containing the `cookie` (transaction ID), `block` flag, `exclusive` flag, and `Nlm4Lock` details. These fields must exactly match the arguments of the outstanding lock request to be canceled.
- `self`: A reference to the implementor of the `Cancel` trait.

Outputs:
- `Nlm4CancelRes`: A structure containing the `cookie` (echoed back) and a `stat` (`Nlm4Stats`) indicating the result of the operation.

Steps:
1.  **Argument Definition**: The `Nlm4CancelArgs` struct is constructed by the RPC layer, deserializing the client's request. It encapsulates the identity of the lock to be canceled.
2.  **Trait Dispatch**: The `Cancel::cancel` method is invoked asynchronously. The implementation must look up a pending (blocked) lock request that matches the provided `cookie`, `block`, `exclusive`, and `lock` fields.
3.  **State Mutation**: If a matching blocked request is found, the implementation removes it from the wait queue.
4.  **Result Formulation**: The implementation returns `Nlm4CancelRes`. The `stat` field is set to `Nlm4Stats::Granted` if the cancellation was successful (the blocked request was removed), or `Nlm4Stats::Denied` if no matching blocked request existed (e.g., the lock was already granted or never existed).

Edge Cases:
- **No Matching Lock**: If the arguments do not correspond to a currently blocked lock, the procedure typically returns `Denied` (or `Granted` depending on specific server interpretation of "cancelling a non-existent wait"), but the interface allows any `Nlm4Stats`.
- **Mismatched Arguments**: The RFC specifies that the arguments must match the original request exactly. If they do not match, the request is effectively a "no-op" regarding the specific intended lock, and the server will likely return `Denied`.

Complexity:
- Time: O(1) for data structure access; the complexity of the `cancel` method implementation depends on the underlying lock manager's data structure (likely O(log N) or O(1) for hash map lookups).
- Space: O(1) for the structs themselves; the implementation manages the lock state.

Determinism:
- Deterministic (The behavior is defined by the trait implementation, but the interface types are pure data).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::nlm::cookie`**:
    - `Cookie`: Provides a type-safe wrapper for the transaction ID. The `raw` method is implicitly used to access the underlying `u64` for comparison or storage.
- **From `nfs_mamont::nlm::lock`**:
    - `Nlm4Lock`: Encapsulates the file handle (`vfs::file::Handle`) and lock range. The `new` constructor ensures valid creation of lock descriptors.
- **From `nfs_mamont::nlm`**:
    - `Nlm4Stats`: Enumerates the status codes (e.g., `Granted`, `Denied`) that populate the `stat` field of the response.

---

## 4. Data Model

Entities:
- `Nlm4CancelArgs`: Represents the input payload for the CANCEL procedure.
- `Nlm4CancelRes`: Represents the output payload for the CANCEL procedure.

Relations:
- `Nlm4CancelArgs` *contains* `Cookie` (1:1).
- `Nlm4CancelArgs` *contains* `Nlm4Lock` (1:1).
- `Nlm4CancelRes` *contains* `Cookie` (1:1).
- `Nlm4CancelRes` *contains* `Nlm4Stats` (1:1).

Global Invariants:
- The `cookie` in `Nlm4CancelRes` must be identical to the `cookie` in `Nlm4CancelArgs` to ensure correct RPC correlation.
- The fields `block`, `exclusive`, and `lock` in `Nlm4CancelArgs` must semantically match the original lock request for the cancellation to be valid per RFC 1813.

---

## 5. Error Model

Error Types:
- None defined in this module.

Error Propagation Strategy:
- Status codes. The `cancel` method returns `Nlm4CancelRes` directly, not a `Result`. Success or failure is communicated via the `stat` field (`Nlm4Stats`), where values like `Denied` or `Failed` indicate errors or inability to perform the cancellation.

Recoverability:
- Recoverability is handled by the client interpreting the `Nlm4Stats` code. For example, receiving `Denied` implies the lock was not in a blocked state or didn't exist, which the client may handle as a "no-op".

Panics:
- Allowed: No.
- Conditions: The module defines only data structures and a trait signature. Panics depend entirely on the implementation of the `Cancel` trait.

---

## 6. Traits

List which external traits this module implements:
- `Send` (via `trait_variant::make`): The `Cancel` trait is transformed to be object-safe and implement `Send`, allowing instances to be shared between threads.

---

## 7. Overview

This section is needed for the evolution of project understanding.
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module.
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed.
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module.
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level.
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface for canceling pending lock requests within the Network Lock Manager (NLM) version 4 protocol. In distributed file systems, clients often request locks that cannot be granted immediately due to conflicts. These requests are "blocked" by the server until the resource becomes available. However, client-side timeouts, user interruptions, or logic changes may require the client to withdraw this waiting request before it is granted.

This system contains the data structures (`Nlm4CancelArgs`, `Nlm4CancelRes`) and the asynchronous trait (`Cancel`) necessary to handle this withdrawal. It relies on the `Cookie` type to correlate the cancellation with the original lock request and `Nlm4Lock` to identify the specific resource and range. The `Nlm4Stats` enum provides the standardized protocol vocabulary to inform the client whether the cancellation was successful.

A typical usage scenario of the system involves a client that previously sent a LOCK request which returned a `Blocked` status. If the client decides to cancel, it constructs a CANCEL request containing the same `cookie`, `block`, `exclusive`, and `lock` details. The server receives this, invokes the `cancel` method on its NLM service implementation. The implementation searches its internal wait queue for a matching request. If found, it removes the request and returns `Nlm4CancelRes` with `stat` set to `Granted` (indicating the cancel operation succeeded). If not found, it returns `Denied`.

Inside the system the following things happen and they use the `Cancel` trait to abstract the specific logic of removing a lock from the server's state machine, ensuring that the RPC handler can operate on a generic `Arc<dyn Cancel>` without knowing the details of the lock storage backend.