<!-- SPEC_HASH: 3ea5e9abb391d115ff3714c902f3f472d37b806031a5e829776bb07ca5051b6f -->
# Module Specification

Module: nfs_mamont::nlm::holder
Rust File: src/nlm/holder.rs

---

## 1. Dependencies

From the source code and context, the following dependencies are identified:

*   **`super::OpaqueHandle`** (from `nfs_mamont::nlm`):
    *   Used to store the host or process-specific identifier of the lock owner. It provides a validated byte container (`Vec<u8>`) that ensures the handle does not exceed the protocol-defined maximum size.
*   **`crate::consts::nlm::OPAQUE_HANDLE_SIZE`** (from `nfs_mamont::consts::nlm`):
    *   Used in the test module to generate dummy data of the correct size for the `OpaqueHandle`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define a data structure that represents the owner of a lock and the specific region of the file they hold, as required by the NLMv4 protocol for reporting lock conflicts (e.g., in `NLM_TEST` responses or `NLM_LOCK` denial responses).

Inputs:
- **`exclusive: bool`**: Indicates if the lock is exclusive (write) or shared (read).
- **`system_identifier: i32`**: The Process ID (PID) of the locking process on the client.
- **`opaque_handle: OpaqueHandle`**: A validated byte sequence identifying the client host or process owner.
- **`lock_offset: u64`**: The starting byte offset of the locked region.
- **`lock_length: u64`**: The length of the locked region in bytes. A value of 0 conventionally means "to end of file".

Outputs:
- **`Nlm4Holder`**: A struct instance encapsulating the lock holder's identity and the lock extent.

Steps:
1.  The `new` associated function is called with the lock parameters.
2.  The function constructs a `Nlm4Holder` instance by directly assigning the input fields to the struct members.
3.  The instance is returned to the caller. No internal validation of the lock range (e.g., overflow checks) is performed within this module; it acts as a passive data carrier.

Edge Cases:
- **Zero Length**: The `lock_length` field accepts 0, which, per the documentation comment, implies the lock extends to the end of the file. The struct itself does not enforce this logic but preserves the value for interpretation by the consumer.
- **Opaque Handle Validation**: While `Nlm4Holder` does not validate the `OpaqueHandle`, the `OpaqueHandle::new` constructor (called before passing the handle to `Nlm4Holder`) ensures the byte vector length is within `OPAQUE_HANDLE_SIZE`.

Complexity:
- **Time**: O(1) for struct initialization.
- **Space**: O(N) where N is the size of the `OpaqueHandle` byte vector.

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::nlm`**:
    - **`OpaqueHandle`**: A structure wrapping a `Vec<u8>` representing a lock owner identifier. It enforces a maximum size constraint (`OPAQUE_HANDLE_SIZE`) upon creation. `Nlm4Holder` uses this to uniquely identify the owner of the lock across the network.

---

## 4. Data Model

Entities:
- **`Nlm4Holder`**: A structure representing the current holder of a lock.

Relations:
- **Composition**: `Nlm4Holder` *contains* one `OpaqueHandle`.

Global Invariants:
- None specific to this module, other than those inherited from the `OpaqueHandle` field (i.e., the handle bytes must be of valid length).

## 5. Error Model

Error Types:
- None defined or returned by the public interface of this module.

Error Propagation Strategy:
- N/A. The `new` constructor returns `Self` directly, not a `Result`. It assumes valid inputs are provided (or that validation occurred at the `OpaqueHandle` creation stage).

Recoverability:
- N/A.

Panics:
- Allowed: No
- Conditions: The public API consists solely of a struct definition and a constructor that performs direct field assignment. There are no operations that induce panics.

---

## 6. Traits

List which external traits this module implements:
- None explicitly visible in the provided code snippet. (Note: In a full implementation, this struct would likely derive traits such as `Debug`, `Clone`, `PartialEq`, and serialization traits like `Serialize`/`Deserialize`, but these are not present in the provided source text).

Traits defined by this module:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to provide a standardized data structure for describing the owner of a file lock within the Network Lock Manager (NLM) version 4 protocol. In distributed file systems, when a client attempts to acquire a lock that conflicts with an existing lock, the server must be able to report exactly *who* holds the conflicting lock and *where* the conflict occurs. The `Nlm4Holder` structure serves this specific purpose by aggregating the lock owner's identity (via `OpaqueHandle` and `system_identifier`) and the lock's extent (offset and length).

The system contains a complex locking mechanism where lock status and conflicts must be communicated back to clients via RPC responses. This module provides the necessary vocabulary for that communication. Specifically, it is used by the NLM procedure implementations (such as `TEST` or `LOCK`) to populate the "holder" field in response structures when a lock request cannot be granted.

A typical usage scenario of the system involves a client sending an `NLM_TEST` request to check if a file region is lockable. The NLM service checks its internal lock state. If a conflict exists, the service constructs a `Nlm4Holder` instance populated with the conflicting lock's details (PID, owner handle, offset, length) and returns it to the client. The client then uses this information to inform the user or manage retry logic. Inside the system, the `Nlm4Holder` relies on the `OpaqueHandle` from the parent module to ensure that the owner identifier is a valid, protocol-compliant byte sequence, preventing malformed data from being propagated to the network layer. Without this module, the NLM service would lack a consistent way to describe lock conflicts, leading to potential ambiguity in error reporting and interoperability issues with NFS clients.