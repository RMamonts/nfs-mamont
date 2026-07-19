<!-- SPEC_HASH: fc74e7387f07f9dca90c3dad6fafe7f117d82058ceb5ca8b3ca42f2db3a07f38 -->
# Module Specification

Module: nfs_mamont::nlm::procedures::unlock
Rust File: src/nlm/procedures/unlock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::cookie`**:
    *   Used to import the `Cookie` type. This type serves as a transaction identifier in both the request arguments (`Nlm4UnlockArgs`) and the result (`Nlm4UnlockRes`) to allow the client to match asynchronous RPC responses with their corresponding requests.
*   **`crate::nlm::lock`**:
    *   Used to import the `Nlm4Lock` type. This structure is embedded within `Nlm4UnlockArgs` to carry the specific details of the lock to be released (caller name, file handle, owner handle, offset, and length).
*   **`crate::nlm`** (parent module):
    *   Used to import the `Nlm4Stats` enum. This type is used in `Nlm4UnlockRes` to indicate the success or failure of the unlock operation (e.g., `Granted` vs `Denied`).
*   **`trait_variant`**:
    *   Used via the `#[trait_variant::make(Send)]` attribute macro. This transforms the async `Unlock` trait into a version that is safe to be used as a trait object (`dyn Unlock`) and can be sent across threads, which is necessary for the asynchronous RPC server architecture.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the data structures and the asynchronous interface required to implement the NLMv4 `UNLOCK` procedure. This module provides the contract that the underlying lock manager must fulfill to process requests to release file locks.

Inputs:
- **`args: Nlm4UnlockArgs`**: Passed to the `Unlock::unlock` method. It contains:
    - `cookie`: A `Cookie` instance identifying the transaction.
    - `lock`: A `Nlm4Lock` instance describing the lock to be removed.

Outputs:
- **`Nlm4UnlockRes`**: Returned by the `Unlock::unlock` method. It contains:
    - `cookie`: The `Cookie` instance echoed back from the request.
    - `stat`: A `Nlm4Stats` enum variant indicating the outcome of the operation.

Steps:
1. **Request Reception**: The RPC layer deserializes the incoming bytes into a `Nlm4UnlockArgs` struct. This struct aggregates the transaction cookie and the lock details.
2. **Trait Dispatch**: The server logic calls the `unlock` method on an object implementing the `Unlock` trait, passing the `Nlm4UnlockArgs`.
3. **Lock Processing**: The implementation of `Unlock` is responsible for locating the lock identified by `args.lock` (matching caller, file handle, owner, offset, and length) and removing it from the lock state.
4. **Response Construction**: The implementation constructs a `Nlm4UnlockRes`. It copies the `cookie` from `args` into the response and sets `stat` to `Nlm4Stats::Granted` if successful, or an appropriate error code (e.g., `Denied`) if the lock did not exist or could not be removed.
5. **Response Transmission**: The `Nlm4UnlockRes` is serialized and sent back to the client.

Edge Cases:
- **Unlocking Non-existent Locks**: The protocol allows unlocking locks that are not held. The implementation typically returns `Granted` in such cases (idempotency), though this behavior is defined by the trait implementor, not this module.
- **Partial Ranges**: The `Nlm4Lock` structure within the arguments defines a specific byte range. The unlock operation is expected to affect only that specific range or the exact lock matching that range.

Complexity:
- **Time**: O(1) for accessing fields of `Nlm4UnlockArgs` and `Nlm4UnlockRes`. The complexity of the actual lock removal depends on the implementation of the `Unlock` trait.
- **Space**: O(1) for the structs themselves (they contain references or owned values of fixed size, excluding the internal allocation of strings/vectors within `Nlm4Lock`).

Determinism:
- Deterministic (regarding data structure definitions).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::nlm::cookie`**:
    - **`Cookie`**: Used as a transaction identifier. The module relies on the `Cookie` type to ensure that the request and response can be correlated by the client.
- **From `nfs_mamont::nlm::lock`**:
    - **`Nlm4Lock`**: Used to specify the target of the unlock operation. The module relies on the fields of `Nlm4Lock` (caller name, file handle, offset, length) to uniquely identify the lock to be released. It assumes that `Nlm4Lock` has already been validated (e.g., `caller_name` length) before being passed to this module.
- **From `nfs_mamont::nlm`**:
    - **`Nlm4Stats`**: Used to encode the status of the operation. The module uses this enum to communicate success (`Granted`) or failure back to the RPC layer.

---

## 4. Data Model

Entities:
- **`Nlm4UnlockArgs`**: A structure representing the arguments for the UNLOCK procedure. It holds the transaction `cookie` and the `lock` details.
- **`Nlm4UnlockRes`**: A structure representing the result of the UNLOCK procedure. It holds the transaction `cookie` and the status `stat`.

Relations:
- **Composition**: `Nlm4UnlockArgs` *contains* one `Cookie` and one `Nlm4Lock`.
- **Composition**: `Nlm4UnlockRes` *contains* one `Cookie` and one `Nlm4Stats`.

Global Invariants:
- The `cookie` field in `Nlm4UnlockRes` must be identical to the `cookie` field in the corresponding `Nlm4UnlockArgs` to ensure correct RPC transaction matching. This invariant must be upheld by the implementer of the `Unlock` trait.

## 5. Error Model

Error Types:
- None defined in this module.

Error Propagation Strategy:
- The module does not use Rust `Result` types for the primary return value of the procedure. Instead, errors (such as "Lock not found" or "Denied") are propagated via the `stat` field of `Nlm4UnlockRes` using the `Nlm4Stats` enum.

Recoverability:
- Recoverable. The client can inspect the `stat` field in the response to determine if the unlock was successful.

Panics:
- Allowed: No.
- Conditions: The module defines only data structures and a trait signature. It contains no executable logic that could panic.

---

## 6. Traits

List which external traits this module implements:
- None.

Traits defined by this module:
- **`Unlock`**: An asynchronous trait marked `Send`. It defines the `unlock` method that must be implemented by the NLM service handler to process unlock requests.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface and data structures required to handle the release of file locks within the Network Lock Manager (NLM) version 4 protocol. In a distributed file system, locks are finite resources that must be explicitly released when a client is finished with a file to allow other clients to access the data. This module provides the specific "vocabulary" (`Nlm4UnlockArgs`, `Nlm4UnlockRes`) and the "action contract" (`Unlock` trait) for this release mechanism.

The system contains a comprehensive implementation of the NLM protocol, where operations are segregated into distinct procedures (Lock, Unlock, Test, Cancel). This module specifically addresses the `UNLOCK` procedure. It relies on the `Nlm4Lock` structure (defined in a dependency) to identify *which* lock to remove, using properties like the file handle, owner ID, and byte range. It relies on `Nlm4Stats` to report the outcome.

A typical usage scenario of the system involves a client closing a file or explicitly releasing a lock. The client sends an `NLMPROC4_UNLOCK` RPC request. The server receives this request and deserializes it into `Nlm4UnlockArgs`. The server then invokes the `unlock` method on its NLM service implementation. Inside the system, the following things happen and they use this module: The service implementation uses the `lock` field from the arguments to look up the lock in its internal state table. If found, it removes the lock. It then constructs a `Nlm4UnlockRes`, copying the `cookie` from the request to the response (ensuring the client can match it) and setting `stat` to `Granted`. This response is sent back to the client. Without this module, the server would lack a standardized way to represent unlock requests and results, making it impossible to interoperate with standard NFS clients expecting the RFC 1813 protocol structure.