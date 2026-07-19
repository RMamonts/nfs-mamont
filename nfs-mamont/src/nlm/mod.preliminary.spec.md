<!-- SPEC_HASH: 8e470dfd4c0f8ec200d9b015b1ce1c97a7d8a8ccef6e62704f44280a6e60cf69 -->
# Module Specification

Module: nfs_mamont::nlm
Rust File: src/nlm/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`std::io`**:
    *   Used to provide the `Error` and `ErrorKind` types for the `OpaqueHandle::new` constructor, allowing it to return a `Result` indicating validation failures (e.g., handle too long).
*   **`num_derive`**:
    *   Used via the `ToPrimitive` and `FromPrimitive` derive macros on the `Nlm4Stats` enum. This enables conversion between the enum variants and their underlying integer representations (u32/i32), which is necessary for encoding/decoding NLM protocol status codes in RPC messages.
*   **`crate::consts::nlm`**:
    *   Used to access the `OPAQUE_HANDLE_SIZE` constant. This constant defines the maximum allowed size for an opaque lock owner identifier, which is enforced by `OpaqueHandle::new`.
*   **`crate::nlm::procedures`** (submodules: `lock`, `unlock`, `test`, `cancel`):
    *   Used to import the specific asynchronous traits (`Lock`, `Unlock`, `Test`, `Cancel`) and their corresponding result types (`Nlm4LockRes`, `Nlm4UnlockRes`, `Nlm4TestRes`, `Nlm4CancelRes`). These are aggregated into the `Nlm` composite trait and the `NlmRes` enum to provide a unified interface for the NLM service.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the foundational data types and the composite service interface for the Network Lock Manager version 4 (NLMv4) protocol. This module acts as a facade, aggregating procedure-specific logic into a single `Nlm` trait and standardizing the representation of status codes, lock owners, and procedure results.

Inputs:
- **`OpaqueHandle::new`**: A `Vec<u8>` representing the raw bytes of a lock owner identifier.
- **`Nlm` trait**: Requires an implementation of the `Lock`, `Unlock`, `Test`, and `Cancel` traits from the submodules.

Outputs:
- **`Nlm4Stats`**: An enum variant representing the success or failure of an NLM operation.
- **`NlmRes`**: An enum variant wrapping the specific result structure of a called procedure.
- **`OpaqueHandle`**: A validated wrapper structure containing the owner identifier bytes.
- **`Nlm` trait**: A trait object that can be used to dispatch any of the four core NLM procedures.

Steps:
1.  **Status Definition**: The `Nlm4Stats` enum is defined with explicit discriminants (0 through 9) corresponding to the status codes specified in RFC 1813 (e.g., `Granted`, `Denied`, `Blocked`).
2.  **Owner Identification**: The `OpaqueHandle::new` function validates the input byte vector. If the length exceeds `OPAQUE_HANDLE_SIZE`, it returns an `io::Error`. Otherwise, it wraps the vector in the `OpaqueHandle` struct.
3.  **Result Aggregation**: The `NlmRes` enum is defined to hold the result of any NLM procedure. This allows a generic RPC handler to return a single type (`NlmRes`) regardless of whether the underlying call was a Lock, Unlock, Test, or Cancel.
4.  **Trait Composition**: The `Nlm` trait is defined as a combination (supertrait) of `procedures::lock::Lock`, `procedures::unlock::Unlock`, `procedures::test::Test`, and `procedures::cancel::Cancel`. A blanket implementation is provided for any type that implements all four sub-traits.

Edge Cases:
- **Opaque Handle Size**: If a client sends an opaque handle larger than `OPAQUE_HANDLE_SIZE`, `OpaqueHandle::new` will fail, causing the request to be rejected before reaching the core lock logic.
- **NlmRes Boxing**: The `Test` variant of `NlmRes` wraps the result in a `Box` (`Box<Nlm4TestRes>`). This is likely an optimization to reduce the stack size of the enum, as `Nlm4TestRes` may be large (containing optional holder information).

Complexity:
- **Time**:
    - `OpaqueHandle::new`: O(N) where N is the length of the input vector (due to the length check).
    - `Nlm4Stats`/`NlmRes` access: O(1).
- **Space**:
    - `OpaqueHandle`: O(N) where N is the length of the handle bytes.
    - `NlmRes`: O(Size of largest variant) due to enum layout.

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::nlm::procedures::lock`**:
    - **`Lock` Trait**: Defines the asynchronous contract for acquiring locks. It is included as a supertrait of `Nlm`.
    - **`Nlm4LockRes`**: The result structure returned by the `Lock` procedure, wrapped in the `NlmRes::Lock` variant.
- **From `nfs_mamont::nlm::procedures::unlock`**:
    - **`Unlock` Trait**: Defines the asynchronous contract for releasing locks. It is included as a supertrait of `Nlm`.
    - **`Nlm4UnlockRes`**: The result structure returned by the `Unlock` procedure, wrapped in the `NlmRes::Unlock` variant.
- **From `nfs_mamont::nlm::procedures::test`**:
    - **`Test` Trait**: Defines the asynchronous contract for polling lock conflicts. It is included as a supertrait of `Nlm`.
    - **`Nlm4TestRes`**: The result structure returned by the `Test` procedure, wrapped in `NlmRes::Test`.
- **From `nfs_mamont::nlm::procedures::cancel`**:
    - **`Cancel` Trait**: Defines the asynchronous contract for canceling blocked requests. It is included as a supertrait of `Nlm`.
    - **`Nlm4CancelRes`**: The result structure returned by the `Cancel` procedure, wrapped in the `NlmRes::Cancel` variant.
- **From `nfs_mamont::consts::nlm`**:
    - **`OPAQUE_HANDLE_SIZE`**: The constant value used to enforce the maximum size of the `OpaqueHandle` byte vector.

---

## 4. Data Model

Entities:
- **`Nlm4Stats`**: An enumeration representing the status of an NLM procedure call (e.g., `Granted`, `Denied`, `Blocked`).
- **`NlmRes`**: An enumeration acting as a sum type for all possible NLM procedure results.
- **`OpaqueHandle`**: A structure wrapping a `Vec<u8>` representing a lock owner identifier, validated for maximum length.
- **`Nlm`**: A composite trait combining `Lock`, `Unlock`, `Test`, and `Cancel`.

Relations:
- **Composition**: `NlmRes` *contains* `Nlm4LockRes`, `Nlm4UnlockRes`, `Box<Nlm4TestRes>`, or `Nlm4CancelRes` (mutually exclusive).
- **Inheritance**: `Nlm` *requires* `Lock`, `Unlock`, `Test`, and `Cancel`.

Global Invariants:
- The byte vector inside any successfully constructed `OpaqueHandle` must have a length less than or equal to `OPAQUE_HANDLE_SIZE`.
- The discriminant values of `Nlm4Stats` must match the values defined in RFC 1813 for NLMv4.

## 5. Error Model

Error Types:
- **`std::io::Error`**

Error Propagation Strategy:
- The module uses `std::io::Result<Self>` for the `OpaqueHandle::new` constructor. Errors are constructed using `std::io::Error::new(std::io::ErrorKind::InvalidInput, "opaque handle too long")`.

Recoverability:
- Recoverable. Callers of `OpaqueHandle::new` (likely RPC deserialization logic) must handle the `Result` to reject malformed requests.

Panics:
- Allowed: No
- Conditions: The public API performs validation and returns errors rather than panicking on invalid input (e.g., oversized vectors).

---

## 6. Traits

List which external traits this module implements:
- **`num_traits::ToPrimitive`**: Implemented for `Nlm4Stats` (via derive).
- **`num_traits::FromPrimitive`**: Implemented for `Nlm4Stats` (via derive).

Traits defined by this module:
- **`Nlm`**: A composite trait requiring the implementation of `Lock`, `Unlock`, `Test`, and `Cancel`.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to provide a unified, protocol-compliant interface for the Network Lock Manager (NLM) version 4 service within the `nfs_mamont` NFS server. The NLM protocol is essential for coordinating file access across a network, preventing data corruption when multiple clients attempt to write to the same resource. This module serves as the root of the NLM subsystem, aggregating the distinct procedure implementations (Lock, Unlock, Test, Cancel) defined in its submodules into a single, coherent service contract.

The system contains a complex implementation of a distributed lock manager where operations are split into specific procedures (e.g., acquiring a lock vs. testing for a conflict). This module defines the `Nlm` trait, which acts as a facade. By requiring the implementation of `Lock`, `Unlock`, `Test`, and `Cancel`, it ensures that any type presented as an NLM service to the main server loop (via `lib.rs`) supports the complete set of locking operations. Additionally, it defines shared data structures like `Nlm4Stats` (the standardized vocabulary for operation results) and `OpaqueHandle` (a validated container for client identifiers), which are used across all procedure modules to ensure consistency.

A typical usage scenario of the system involves the main server loop receiving an RPC request. The dispatcher identifies the request as an NLM procedure. It invokes the appropriate method on an object implementing the `Nlm` trait (e.g., `nlm_service.lock(args)`). The result, which might be a `Nlm4LockRes`, is wrapped in the `NlmRes::Lock` enum variant. This allows the RPC layer to handle the return value generically, regardless of which specific procedure was called. Inside the system, the `OpaqueHandle` ensures that any client identifier received over the network is validated against the `OPAQUE_HANDLE_SIZE` constant before being processed, preventing potential buffer overflow issues or protocol violations in the underlying lock storage logic. Without this module, the NLM procedures would be disjointed, requiring the RPC layer to manage multiple independent traits and data types, increasing complexity and the risk of implementation drift from the RFC 1813 standard.