<!-- SPEC_HASH: 9bcbb7e1f95263fc4c68d8cee8320c448c7906a9722ed473a2b3e153bd6a7c07 -->
# Module Specification

Module: nfs_mamont::nlm::lock
Rust File: src/nlm/lock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**:
    - Used to provide the `Error` and `ErrorKind` types. The `Nlm4Lock::new` constructor returns a `std::io::Result` to signal validation failures (e.g., empty or too-long caller names) using the `InvalidInput` error kind.
- **`crate::consts::nlm`**:
    - Used to access the `LM_MAXSTRLEN` constant. This constant defines the maximum allowed length for the `caller_name` field, which is enforced by `Nlm4Lock::new`.
- **`crate::vfs`**:
    - Used to access the `vfs::file::Handle` type. This type is used within `Nlm4Lock` to identify the specific file on which the lock operation is to be performed.
- **`crate::nlm` (parent module)**:
    - Used to access the `OpaqueHandle` type. This type is used within `Nlm4Lock` to identify the specific lock owner (host or process) making the request.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define a validated data structure (`Nlm4Lock`) that represents a lock request in the Network Lock Manager version 4 (NLMv4) protocol. This structure encapsulates all necessary parameters (client identity, file handle, lock range, owner) and enforces protocol-level constraints on the client name before the request is processed by the locking logic.

Inputs:
- **`caller_name`**: A `String` representing the hostname of the client requesting the lock.
- **`file_handle`**: A `vfs::file::Handle` identifying the file to be locked.
- **`opaque_handle`**: An `OpaqueHandle` identifying the owner of the lock.
- **`system_identifier`**: An `i32` representing the PID of the process requesting the lock.
- **`lock_offset`**: A `u64` representing the byte offset where the lock region begins.
- **`lock_length`**: A `u64` representing the length of the lock region in bytes.

Outputs:
- **`Nlm4Lock`**: A struct instance containing the validated lock request parameters.
- **`std::io::Error`**: An error returned if validation of the `caller_name` fails.

Steps:
1. **Construction Request**: The `Nlm4Lock::new` function is invoked with the lock parameters.
2. **Caller Name Validation**:
    - The function checks if `caller_name` is empty. If it is, it returns an `Err` with `ErrorKind::InvalidInput`.
    - The function checks if the length of `caller_name` exceeds `nlm::LM_MAXSTRLEN`. If it does, it returns an `Err` with `ErrorKind::InvalidInput`.
3. **Instantiation**: If all validation checks pass, the function constructs the `Nlm4Lock` struct, initializing all fields with the provided arguments, and returns it wrapped in `Ok`.

Edge Cases:
- **Empty Caller Name**: The constructor explicitly rejects an empty string for `caller_name`, preventing anonymous or unidentified lock requests as per protocol requirements.
- **Oversized Caller Name**: The constructor rejects strings longer than `LM_MAXSTRLEN`, ensuring the identifier fits within the buffer limits expected by the NLM protocol.

Complexity:
- **Time**: O(N) where N is the length of `caller_name` (due to the length check).
- **Space**: O(1) additional space (excluding the storage required for the struct itself).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::consts::nlm`**:
    - **`LM_MAXSTRLEN`**: This constant provides the upper bound for the `caller_name` validation logic in `Nlm4Lock::new`. It ensures that the module adheres to the specific string length limits defined by the NLM protocol constants.

- **From `nfs_mamont::vfs::file`**:
    - **`Handle`**: This structure is used as a field in `Nlm4Lock` to uniquely identify the file involved in the lock request. It relies on the fixed-size array definition provided by the VFS module.

- **From `nfs_mamont::nlm`**:
    - **`OpaqueHandle`**: This structure is used as a field in `Nlm4Lock` to represent the lock owner. It encapsulates a validated byte vector, ensuring that the owner identifier passed to the lock structure has already passed basic size validation.

---

## 4. Data Model

Entities:
- **`Nlm4Lock`**: A structure representing a lock request.
    - `caller_name`: `String` — The name of the client host.
    - `file_handle`: `vfs::file::Handle` — The handle of the file to lock.
    - `opaque_handle`: `OpaqueHandle` — The identifier for the lock owner.
    - `system_identifier`: `i32` — The PID of the requesting process.
    - `lock_offset`: `u64` — The starting byte of the lock region.
    - `lock_length`: `u64` — The length of the lock region (0 means to EOF).

Relations:
- **Composition**: `Nlm4Lock` is composed of `vfs::file::Handle` and `OpaqueHandle`.

Global Invariants:
- For any instance of `Nlm4Lock` created via `new`, the `caller_name` field is guaranteed to be non-empty and have a length less than or equal to `LM_MAXSTRLEN`.

## 5. Error Model

Error Types:
- **`std::io::Error`**

Error Propagation Strategy:
- The module uses the `Result` type for the `Nlm4Lock::new` constructor. Errors are constructed using `std::io::Error::new(std::io::ErrorKind::InvalidInput, "message")`.

Recoverability:
- Recoverable. Callers (typically RPC deserialization handlers) can check the `Result` and reject the request or return a specific NLM status code (e.g., `Denied`) to the client if validation fails.

Panics:
- Allowed: No
- Conditions: The public API performs explicit validation and returns errors rather than panicking on invalid input.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **define and validate the payload for lock requests** within the Network Lock Manager (NLM) version 4 subsystem of the `nfs_mamont` server. The system contains a complex implementation of a distributed file locking mechanism where clients request exclusive or shared access to regions of files. This module serves as the primary data structure for these requests, ensuring that the data entering the core locking logic conforms to the NLM protocol constraints before any state changes are attempted.

A typical usage scenario of the system involves the RPC layer receiving a `LOCK` request from a client. The RPC layer deserializes the raw bytes into primitive types (String, Vec, integers). It then invokes `Nlm4Lock::new` to assemble these primitives into a structured request. This constructor acts as a gatekeeper: it verifies that the `caller_name` is not empty and does not exceed the maximum length defined by the protocol (`LM_MAXSTRLEN`). If validation fails, the request is rejected immediately, preventing invalid data from reaching the lock manager's state machine. Once validated, the `Nlm4Lock` instance, containing the `vfs::file::Handle` (identifying the file) and `OpaqueHandle` (identifying the owner), is passed to the asynchronous lock handler.

Inside the system, the following things happen and they use this module:
1. **Protocol Enforcement**: The module enforces the `LM_MAXSTRLEN` limit defined in `consts::nlm`. This is critical because the NLM protocol specifies maximum string sizes for interoperability; violating these could cause buffer overflows in clients or servers adhering strictly to the spec.
2. **Type Safety**: By wrapping the raw lock parameters in the `Nlm4Lock` struct, the module ensures that all subsequent logic in the lock procedures (e.g., `procedures::lock`) receives a complete and validated set of arguments. This eliminates the need for repetitive validation checks deeper in the call stack.
3. **Integration with VFS**: The module bridges the NLM protocol and the Virtual File System (VFS) by including `vfs::file::Handle`. This allows the lock manager to operate on the same file identifiers used by the NFS data operations, ensuring that locks are applied to the correct files.

Without this module, the lock procedures would have to accept loose, unvalidated parameters, scattering validation logic (like checking string lengths) throughout the codebase. This would increase the risk of bugs where invalid lock requests corrupt the lock state or violate protocol assumptions. This module centralizes the definition and validation of the lock request, providing a robust foundation for the locking service.