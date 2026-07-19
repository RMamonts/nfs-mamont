<!-- SPEC_HASH: fc74e7387f07f9dca90c3dad6fafe7f117d82058ceb5ca8b3ca42f2db3a07f38 -->
# Module Specification

Module: nfs_mamont::nlm::procedures::unlock
Rust File: src/nlm/procedures/unlock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::cookie::Cookie`**: Used to provide a transaction identifier. This allows the NLM protocol to match asynchronous requests with their corresponding responses, ensuring the client can correlate the result with the specific operation it initiated.
*   **`crate::nlm::lock::Nlm4Lock`**: Used to encapsulate the specific parameters required to identify a lock. This includes the file handle, the owner (caller name), and the byte range (offset/length), ensuring the correct lock is targeted for removal.
*   **`crate::nlm::Nlm4Stats`**: Used to define the status of the unlock operation. It provides a standardized set of codes (e.g., `Granted`, `Denied`) to indicate success or specific reasons for failure to the client.
*   **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` macro. This allows the asynchronous `Unlock` trait to be converted into a trait object that is also `Send`, enabling dynamic dispatch across thread boundaries in an asynchronous runtime (e.g., Tokio).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the data structures and the asynchronous interface required to implement the NLMv4 `UNLOCK` procedure. This module abstracts the network protocol details into Rust types, separating the "what" (arguments and results) from the "how" (the implementation of the lock removal logic).

Inputs:
- `args: Nlm4UnlockArgs`: A structure containing the `cookie` (transaction ID) and `lock` (details of the lock to be removed).
- `&self`: A reference to the implementer of the `Unlock` trait, which holds the state or context necessary to modify the lock manager.

Outputs:
- `Nlm4UnlockRes`: A structure containing the `cookie` (echoed back to the client) and a `stat` (`Nlm4Stats`) indicating the outcome of the operation.

Steps:
1.  **Request Reception**: The system receives an `Nlm4UnlockArgs` struct. This struct aggregates the transaction context (`cookie`) and the lock identification data (`Nlm4Lock`).
2.  **Trait Dispatch**: The `unlock` method is invoked on an object implementing the `Unlock` trait. Due to the `#[trait_variant::make(Send)]` attribute, this invocation is thread-safe and compatible with async runtimes.
3.  **Execution**: The concrete implementation of `Unlock` performs the logic to locate and remove the lock specified by `args.lock` (identified by caller, file handle, offset, and length).
4.  **Response Construction**: The implementation returns an `Nlm4UnlockRes`. This struct must contain the same `cookie` received in the arguments to satisfy the NLM protocol requirements for request-response matching, along with a status code.

Edge Cases:
- **Unlocking a non-existent lock**: The NLM protocol typically treats unlocking a non-existent lock as a success (idempotent operation), so the implementation should likely return `Nlm4Stats::Granted`.
- **Invalid Ranges**: If the `lock_offset` or `lock_length` in `Nlm4Lock` are invalid (e.g., overflow), the implementation may return `Nlm4Stats::Fbig` or `Nlm4Stats::Failed`.

Complexity:
- Time: O(1) for data structure access; the complexity of the actual unlock operation depends on the implementation of the `Unlock` trait.
- Space: O(1) for the argument and result structures (excluding the size of internal strings/vectors within `Nlm4Lock`).

Determinism:
- Deterministic (regarding the interface definition). The specific behavior depends on the implementation of the `Unlock` trait.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **`Cookie` (from `nfs_mamont::nlm::cookie`)**: Provides the `new` constructor and `raw` accessor. While this module primarily holds the `Cookie`, the dependency ensures that the transaction ID is a distinct type rather than a raw `u64`, preventing confusion with other numeric identifiers.
- **`Nlm4Lock` (from `nfs_mamont::nlm::lock`)**: Provides the detailed lock context. Its fields (`caller_name`, `file_handle`, `lock_offset`, etc.) are essential for the `Unlock` implementation to uniquely identify the lock to be removed within the global lock table.
- **`Nlm4Stats` (from `nfs_mamont::nlm`)**: Provides the enumeration of status codes. The `Unlock` procedure relies on this enum to communicate success (`Granted`) or failure (`Denied`, `StaleFh`, etc.) back to the client.

---

## 4. Data Model

Entities:
- `Nlm4UnlockArgs`: A structure representing the input arguments for an unlock request. It holds a `Cookie` and an `Nlm4Lock`.
- `Nlm4UnlockRes`: A structure representing the result of an unlock request. It holds a `Cookie` and an `Nlm4Stats`.

Relations:
- `Nlm4UnlockArgs` contains `Cookie` (Composition).
- `Nlm4UnlockArgs` contains `Nlm4Lock` (Composition).
- `Nlm4UnlockRes` contains `Cookie` (Composition).
- `Nlm4UnlockRes` contains `Nlm4Stats` (Composition).

Global Invariants:
- **Cookie Echoing**: The `cookie` field in `Nlm4UnlockRes` must be identical to the `cookie` field in the corresponding `Nlm4UnlockArgs`. This is a strict requirement of the NLM protocol for RPC correlation.
- **Lock Identification**: The `Nlm4Lock` struct within `Nlm4UnlockArgs` must contain sufficient information (handle, owner, range) to uniquely identify a lock in the system.

## 5. Error Model

Error Types:
- None defined in this module. Error conditions are not represented as Rust `Result` types or custom error enums within the scope of this file.

Error Propagation Strategy:
- Status Codes. Errors are propagated via the `stat` field of the `Nlm4UnlockRes` struct, utilizing the `Nlm4Stats` enum (e.g., `DeniedNolocks`, `StaleFh`).

Recoverability:
- Recoverability is determined by the specific `Nlm4Stats` value returned. For example, `DeniedGracePeriod` implies the client should retry later, while `StaleFh` implies the file handle is invalid and the operation is unrecoverable without a new handle.

Panics:
- Allowed: No.
- Conditions: The module consists solely of struct definitions and a trait definition. There is no executable code in this module that could panic.

---

## 6. Traits

List which external traits this module implements:
- `Send`: Implemented for the `Unlock` trait via the `#[trait_variant::make(Send)]` macro. This allows the trait object to be sent between threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the contract for releasing file locks within a Network Lock Manager (NLM) v4 implementation. The NLM protocol is crucial for Network File Systems (NFS) to manage concurrent access to files across a network, preventing data corruption when multiple clients attempt to write to the same resource.

This system contains a strongly-typed representation of the `UNLOCK` procedure arguments and results, decoupling the network protocol logic from the actual lock management backend. By defining the `Unlock` trait, the system allows the underlying lock storage mechanism (which might be in-memory, persistent, or distributed) to be swapped or mocked without affecting the RPC handling layer. The use of `#[trait_variant::make(Send)]` ensures that this unlock operation can be safely executed in a multi-threaded asynchronous server environment.

A typical usage scenario of the system involves an NFS client sending an `UNLOCK` request to the server after finishing work on a file. The server deserializes the request into `Nlm4UnlockArgs`. It then invokes the `unlock` method on a service implementing the `Unlock` trait. This service uses the `Nlm4Lock` details (specifically the `file_handle` and `caller_name`) to locate the lock in its internal state and removes it. Finally, the service returns `Nlm4UnlockRes` with a status of `Granted`, which the server serializes and sends back to the client.

Inside the system the following things happen and they use the `Cookie` type to ensure the response is correctly matched to the request, and the `Nlm4Stats` type to provide a standardized, protocol-compliant status report, ensuring interoperability with diverse NFS clients.