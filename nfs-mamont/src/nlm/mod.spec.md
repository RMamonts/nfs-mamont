<!-- SPEC_HASH: 8e470dfd4c0f8ec200d9b015b1ce1c97a7d8a8ccef6e62704f44280a6e60cf69 -->
# Module Specification

Module: nfs_mamont::nlm
Rust File: src/nlm/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`std::io`**:
    *   Used to provide the `Error` and `ErrorKind` types. The `OpaqueHandle::new` constructor returns `std::io::Result` to signal validation failures (specifically `InvalidInput`) when the provided byte vector exceeds the maximum allowed size.
*   **`num_derive::{FromPrimitive, ToPrimitive}`**:
    *   Used as derive macros on the `Nlm4Stats` enum. These macros implement the `FromPrimitive` and `ToPrimitive` traits from the `num_traits` crate, enabling conversion between the enum variants and their underlying integer representations (i32/u32), which is necessary for serialization/deserialization of NLM protocol status codes.
*   **`crate::consts::nlm::OPAQUE_HANDLE_SIZE`**:
    *   Used to define the maximum allowable length (in bytes) for an `OpaqueHandle`. The `OpaqueHandle::new` method uses this constant to validate that the lock owner identifier does not exceed the protocol-defined limit.
*   **`crate::nlm::procedures::{cancel, lock, test, unlock}`**:
    *   Used to import the specific result types (`Nlm4CancelRes`, `Nlm4LockRes`, `Nlm4TestRes`, `Nlm4UnlockRes`) and procedure traits (`Cancel`, `Lock`, `Test`, `Unlock`). These are aggregated into the `NlmRes` enum and the composite `Nlm` trait to provide a unified interface for the NLM service.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Protocol Status Enumeration (`Nlm4Stats`)

**Intent:**
To define the complete set of status codes used in the NLMv4 protocol to indicate the outcome of locking operations (success, denial, blocking, errors).

**Inputs:**
None (Compile-time enum definition).

**Outputs:**
Enum variants representing specific states (e.g., `Granted`, `Denied`, `Blocked`).

**Steps:**
1.  The enum is defined with explicit discriminants (0 through 9) corresponding to RFC 1813 specifications.
2.  Derive macros generate implementations for `Debug`, `Copy`, `Clone`, `PartialEq`, `Eq`, `ToPrimitive`, and `FromPrimitive`.

**Edge Cases:**
None. The enum is a pure data definition.

**Complexity:**
Time: O(1) for access/derivation.
Space: O(1) (size of a primitive integer).

**Determinism:**
Deterministic.

### Mechanism 2: Validated Lock Owner Identifier (`OpaqueHandle`)

**Intent:**
To encapsulate a byte sequence representing a lock owner (client/host) while enforcing the NLMv4 size constraint on the identifier at the point of creation.

**Inputs:**
`oh: Vec<u8>`: A byte vector representing the raw owner handle.

**Outputs:**
`io::Result<OpaqueHandle>`: The wrapped handle or an error if the input is too long.

**Steps:**
1.  The `new` function is called with a `Vec<u8>`.
2.  The function checks if `oh.len() > OPAQUE_HANDLE_SIZE`.
3.  If the check fails, it returns `Err(io::Error::new(io::ErrorKind::InvalidInput, "opaque handle too long"))`.
4.  If the check passes, it wraps the vector in the `OpaqueHandle` struct and returns `Ok(Self)`.

**Edge Cases:**
*   **Empty Vector**: Allowed (length 0 <= `OPAQUE_HANDLE_SIZE`).
*   **Exact Maximum Size**: Allowed.

**Complexity:**
Time: O(1) (length check is constant time relative to the limit, though technically O(N) on the vector length to calculate `len()`).
Space: O(N) where N is the size of the byte vector.

**Determinism:**
Deterministic.

### Mechanism 3: Composite Service Trait (`Nlm`)

**Intent:**
To provide a single trait alias that aggregates all individual NLM procedure traits (`Lock`, `Unlock`, `Test`, `Cancel`). This simplifies generic constraints by allowing the server to depend on one `Nlm` trait instead of four separate ones.

**Inputs:**
Type `T` that implements `procedures::lock::Lock`, `procedures::unlock::Unlock`, `procedures::test::Test`, and `procedures::cancel::Cancel`.

**Outputs:**
Implementation of `Nlm` for type `T`.

**Steps:**
1.  The `Nlm` trait is defined with a supertrait list requiring `Lock`, `Unlock`, `Test`, and `Cancel`.
2.  A blanket implementation `impl<T> Nlm for T where T: Lock + Unlock + Test + Cancel` is provided, automatically implementing `Nlm` for any type satisfying the component traits.

**Edge Cases:**
None. This is a compile-time construct.

**Complexity:**
Time: O(1) (Compile-time resolution).
Space: O(0) (Zero-cost abstraction).

**Determinism:**
Deterministic.

### Mechanism 4: Procedure Result Aggregation (`NlmRes`)

**Intent:**
To define a single enum type that can hold the result of any NLMv4 procedure. This is useful for RPC dispatchers or handlers that need to return or process results polymorphically.

**Inputs:**
Specific result structs (`Nlm4LockRes`, `Nlm4UnlockRes`, etc.).

**Outputs:**
`NlmRes` enum variant wrapping the specific result.

**Steps:**
1.  The enum `NlmRes` is defined with variants `Null`, `Lock`, `Unlock`, `Test`, and `Cancel`.
2.  The `Test` variant specifically wraps the result in a `Box` (`Box<Nlm4TestRes>`) to manage the size of the struct, as `Nlm4TestRes` might be large.

**Edge Cases:**
*   **Null Procedure**: Represented by the `Null` variant with no associated data.

**Complexity:**
Time: O(1) for wrapping/unwrapping.
Space: O(Size of largest variant + discriminant).

**Determinism:**
Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

*   **From `nfs_mamont::nlm::procedures::lock`**:
    *   **`Nlm4LockRes`**: Used as the associated data for the `NlmRes::Lock` variant.
    *   **`Lock` Trait**: Used as a component of the composite `Nlm` trait, ensuring the service can acquire locks.
*   **From `nfs_mamont::nlm::procedures::unlock`**:
    *   **`Nlm4UnlockRes`**: Used as the associated data for the `NlmRes::Unlock` variant.
    *   **`Unlock` Trait**: Used as a component of the composite `Nlm` trait, ensuring the service can release locks.
*   **From `nfs_mamont::nlm::procedures::test`**:
    *   **`Nlm4TestRes`**: Used as the associated data for the `NlmRes::Test` variant (boxed).
    *   **`Test` Trait**: Used as a component of the composite `Nlm` trait, ensuring the service can poll lock status.
*   **From `nfs_mamont::nlm::procedures::cancel`**:
    *   **`Nlm4CancelRes`**: Used as the associated data for the `NlmRes::Cancel` variant.
    *   **`Cancel` Trait**: Used as a component of the composite `Nlm` trait, ensuring the service can cancel pending requests.
*   **From `nfs_mamont::consts::nlm`**:
    *   **`OPAQUE_HANDLE_SIZE`**: Provides the upper bound validation logic for `OpaqueHandle::new`.

---

## 4. Data Model

Entities:
*   **`Nlm4Stats`**: An enumeration representing the status of an NLM procedure call (e.g., `Granted`, `Denied`).
*   **`NlmRes`**: An enumeration acting as a wrapper for the result types of all NLM procedures.
*   **`OpaqueHandle`**: A structure wrapping a `Vec<u8>` representing a lock owner identifier, validated for length.
*   **`Nlm`**: A composite trait combining `Lock`, `Unlock`, `Test`, and `Cancel` traits.

Relations:
*   **Composition**: `NlmRes` *contains* `Nlm4LockRes`, `Nlm4UnlockRes`, `Box<Nlm4TestRes>`, or `Nlm4CancelRes`.
*   **Dependency**: `Nlm` *depends on* `Lock`, `Unlock`, `Test`, and `Cancel`.

Global Invariants:
*   For any instance of `OpaqueHandle` created via `new`, the length of the internal byte vector is guaranteed to be less than or equal to `OPAQUE_HANDLE_SIZE`.
*   The discriminant values of `Nlm4Stats` must match the integer values defined in RFC 1813 for NLMv4.

## 5. Error Model

Error Types:
*   **`std::io::Error`**

Error Propagation Strategy:
*   **Constructor Validation**: The `OpaqueHandle::new` function returns `std::io::Result<Self>`. If the input vector is too long, it returns an error with `ErrorKind::InvalidInput`.

Recoverability:
*   **Recoverable**: Callers (typically RPC deserialization layers) can catch the `Err` result and reject the request or return a protocol-specific error (e.g., `Nlm4Stats::Failed`) to the client.

Panics:
*   Allowed: No.
*   Conditions: The public API performs explicit checks and returns `Result` types rather than panicking on invalid input.

---

## 6. Traits

List which external traits this module implements:
*   **`ToPrimitive`**: Implemented for `Nlm4Stats` (via `num_derive`).
*   **`FromPrimitive`**: Implemented for `Nlm4Stats` (via `num_derive`).

Traits defined by this module:
*   **`Nlm`**: A composite trait requiring the implementation of `Lock`, `Unlock`, `Test`, and `Cancel` procedures. It serves as the main interface for an NLMv4 service.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **define the core data types and the unified service interface for the Network Lock Manager version 4 (NLMv4)**. The system contains a distributed file server (NFS) that requires a robust locking mechanism to ensure data consistency across multiple clients. This module acts as the central aggregation point for the NLM subsystem, bringing together the individual procedure implementations (lock, unlock, test, cancel) defined in submodules under a single, coherent API.

The system relies on this module to provide the "vocabulary" of the protocol—specifically the `Nlm4Stats` enum which standardizes how success and failure are communicated, and the `OpaqueHandle` which ensures that client identifiers adhere to protocol size limits. Furthermore, the `Nlm` trait defined here is crucial for the server's architecture: it allows the main server loop (defined in `lib.rs`) to treat the entire locking service as a single, swappable component (`Arc<dyn Nlm>`) rather than managing a disjoint collection of handlers.

A typical usage scenario of the system involves the RPC layer receiving a request. It dispatches this request to a specific handler (e.g., `Lock`). The handler uses `Nlm4Stats` to construct the response status. If the request involves identifying a lock owner, `OpaqueHandle` is used to safely wrap the incoming bytes. The server implementation itself implements the `Nlm` trait, thereby guaranteeing that it supports all necessary locking operations.

Inside the system, the following things happen and they use this module:
1.  **Type Safety**: By defining `OpaqueHandle` with a constructor that validates against `OPAQUE_HANDLE_SIZE`, the module ensures that invalid data is rejected at the boundary of the NLM service, preventing protocol violations deeper in the stack.
2.  **Interface Simplification**: The `Nlm` trait combines the four procedure traits (`Lock`, `Unlock`, `Test`, `Cancel`). This allows dependency injection in the main server to be concise (`N: Nlm`), enforcing that any injected service must provide a complete locking implementation.
3.  **Result Polymorphism**: The `NlmRes` enum allows the RPC layer to handle the return values of different procedures generically if necessary, or simply provides a namespace for all possible result types.

Without this module, the NLM implementation would lack a unified entry point, forcing the higher-level server logic to depend directly on specific procedure modules. This would increase coupling and make it difficult to ensure that all required procedures are implemented or to swap the locking implementation for testing or different versions.