<!-- SPEC_HASH: 9bcbb7e1f95263fc4c68d8cee8320c448c7906a9722ed473a2b3e153bd6a7c07 -->
# Module Specification

Module: nfs_mamont::nlm::lock
Rust File: src/nlm/lock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **std::io**: Used to provide the `Error` type and `ErrorKind` enum for reporting validation failures (specifically `InvalidInput`) when constructing a `Nlm4Lock` instance.
- **crate::consts::nlm**: Used to access the `LM_MAXSTRLEN` constant, which defines the maximum allowed length for the `caller_name` field in a lock request.
- **crate::vfs**: Used to access the `file::Handle` type, which serves as the identifier for the file on which the lock operation is to be performed.
- **crate::nlm (parent module)**: Used to import the `OpaqueHandle` type, which represents the client-side lock owner identifier.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define a data structure (`Nlm4Lock`) that encapsulates all parameters required for a Network Lock Manager version 4 (NLMv4) lock request.
- To enforce protocol-level validation on the `caller_name` field during the instantiation of `Nlm4Lock`, ensuring it adheres to the length constraints defined by the NLM specification.

Inputs:
- `caller_name`: A `String` representing the hostname of the client requesting the lock.
- `file_handle`: A `vfs::file::Handle` identifying the target file.
- `opaque_handle`: An `OpaqueHandle` identifying the specific owner of the lock on the client side.
- `system_identifier`: An `i32` representing the process ID (PID) of the requester.
- `lock_offset`: A `u64` specifying the starting byte of the region to lock.
- `lock_length`: A `u64` specifying the length of the region to lock (0 implies locking to the end of the file).

Outputs:
- A `Result<Self, Error>` containing the initialized `Nlm4Lock` instance on success, or a `std::io::Error` on validation failure.

Steps:
1. The `Nlm4Lock::new` function is called with the lock request parameters.
2. The function validates the `caller_name`:
   - It checks if the string is empty. If true, it returns an `Error` with kind `InvalidInput`.
   - It checks if the string length exceeds `nlm::LM_MAXSTRLEN`. If true, it returns an `Error` with kind `InvalidInput`.
3. If all validations pass, the function constructs the `Nlm4Lock` struct with the provided fields and returns it wrapped in `Ok`.

Edge Cases:
- A `caller_name` with a length exactly equal to `LM_MAXSTRLEN` is accepted.
- A `caller_name` with a length of `LM_MAXSTRLEN + 1` is rejected.
- A `lock_length` of 0 is treated as a special case meaning "lock to end of file" (as per documentation), though no specific logic is applied to transform this value during construction; it is stored as-is.

Complexity:
- Time: O(N) where N is the length of `caller_name` (due to the length check).
- Space: O(N) where N is the size of the `caller_name` string and the internal buffer of `opaque_handle`.

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::consts::nlm`**: The constant `LM_MAXSTRLEN` is utilized as the threshold for validating the `caller_name` field length.
- **From `crate::vfs::file`**: The `Handle` type is used to store the file identifier. It is assumed to be a fixed-size array of bytes representing an NFSv3 file handle.
- **From `crate::nlm` (parent module)**: The `OpaqueHandle` type is used to store the lock owner identifier. It is assumed to wrap a vector of bytes (`Vec<u8>`) and provides a constructor `new` and an accessor `as_bytes`.

---

## 4. Data Model

Entities:
- **Nlm4Lock**: A structure representing a lock request in the NLMv4 protocol. It aggregates the client identity, file handle, lock owner handle, system ID, and the byte range (offset/length) for the lock.

Relations:
- **Composition**: `Nlm4Lock` contains a `vfs::file::Handle`.
- **Composition**: `Nlm4Lock` contains an `OpaqueHandle`.

Global Invariants:
- For any instance of `Nlm4Lock` successfully created via `new`, the `caller_name` field is guaranteed to be non-empty and its length is guaranteed to be less than or equal to `nlm::LM_MAXSTRLEN`.

## 5. Error Model

Error Types:
- `std::io::Error`

Error Propagation Strategy:
- The module uses `std::io::Result<Self>` for the `Nlm4Lock::new` constructor. Errors are constructed using `Error::new(std::io::ErrorKind::InvalidInput, message)`.

Recoverability:
- Recoverable. Callers of `Nlm4Lock::new` must handle the `Result` to determine if the request parameters are valid before proceeding with lock acquisition logic.

Panics:
- Allowed: No
- Conditions: The public API performs no operations that can result in a panic (e.g., no array indexing, no `unwrap()` calls).

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to provide a structured and validated representation of lock requests within the Network Lock Manager (NLM) version 4 protocol implementation. The system contains an NFS server that requires a mechanism to manage file locks across the network to prevent data corruption when multiple clients access shared files. A typical usage scenario of the system involves the server receiving an RPC request for an NLM `LOCK` procedure. The server decodes the request arguments and invokes `Nlm4Lock::new` to construct a lock object. Inside the system, the following things happen and they use this module: The `Nlm4Lock` struct serves as the primary data carrier for lock operations, passing validated information about the caller (hostname), the target file (via `vfs::Handle`), and the specific lock owner (via `OpaqueHandle`) to the core locking logic. By validating the `caller_name` against `LM_MAXSTRLEN` at the entry point, the system ensures that malformed requests are rejected early, preventing buffer overflows or protocol violations in downstream processing. Without this module, the lock handling procedures would need to implement ad-hoc validation for every request, leading to code duplication and potential security vulnerabilities.