<!-- SPEC_HASH: 40a79b1cb4d8f2bfa0e39fc68e4a9d27419070a22e34e009d88160447006abf5 -->
# Module Specification

Module: nfs_mamont::parser::nlm
Rust File: src/parser/nlm/mod.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`crate::consts::nlm`**: Used to access protocol constants such as `LM_MAXSTRLEN` (maximum string length for caller names) and `OPAQUE_HANDLE_SIZE` (maximum size for lock owner identifiers). These constants are used to enforce validation limits during parsing.
- **`crate::nlm::lock::Nlm4Lock`**: Used as the target data structure for the `parse_lock` function. This struct represents the shared lock arguments block used across NLM procedures.
- **`crate::nlm::OpaqueHandle`**: Used as the target data structure for the `opaque_handle` function. This struct wraps the lock owner identifier bytes.
- **`crate::parser::nfsv3::file`**: Used to parse the file handle field within the lock arguments. NLMv4 reuses the NFSv3 file handle format, so this module delegates the parsing of that specific field to the NFSv3 parser.
- **`crate::parser::primitive`**: Used to read low-level XDR-encoded data types from the byte stream. Specifically, it uses `vector` for opaque data, `string_max_size` for the caller name, and `i32`/`u64` for numeric fields.
- **`crate::parser::{Error, Result}`**: Used for error handling and return types. The module converts specific validation errors (e.g., oversized handles) into the `Error::BadFileHandle` variant.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide the parsing logic for the common data structures shared by all NLMv4 procedures (the lock owner handle and the lock arguments block).
- To act as a namespace aggregator for the parsers of specific NLM procedures (`cancel`, `lock`, `test`, `unlock`), which are implemented in submodules.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream (e.g., network socket or buffer) containing the XDR-encoded NLM arguments.

Outputs:
- `Result<OpaqueHandle>`: A wrapper containing the validated lock owner identifier bytes.
- `Result<Nlm4Lock>`: A structure containing the validated lock details (caller name, file handle, owner, system ID, offset, length).

Steps:
1. **Opaque Handle Parsing (`opaque_handle`)**:
 - The function invokes `primitive::vector(src)` to read a variable-length byte array from the stream.
 - It attempts to construct an `OpaqueHandle` from these bytes using `OpaqueHandle::new`.
 - If the constructor fails (e.g., the byte vector exceeds `OPAQUE_HANDLE_SIZE`), the error is mapped to `Error::BadFileHandle`.
2. **Lock Arguments Parsing (`parse_lock`)**:
 - The function reads the `caller_name` using `primitive::string_max_size(src, nlm::LM_MAXSTRLEN)`.
 - It reads the `file_handle` using `file::handle(src)`.
 - It reads the `opaque_handle` using the `opaque_handle(src)` function defined in this module.
 - It reads the `system_identifier` (svid) using `primitive::i32(src)`.
 - It reads the `lock_offset` using `primitive::u64(src)`.
 - It reads the `lock_length` using `primitive::u64(src)`.
 - It attempts to construct an `Nlm4Lock` struct from these fields using `Nlm4Lock::new`.
 - If the constructor fails (e.g., invalid caller name), the error is mapped to `Error::BadFileHandle`.

Edge Cases:
- **Empty Caller Name**: If the caller name string is empty, `Nlm4Lock::new` will return an error, which `parse_lock` converts to `Error::BadFileHandle`.
- **Oversized Data**: If the opaque handle exceeds `OPAQUE_HANDLE_SIZE` or the caller name exceeds `LM_MAXSTRLEN`, the respective constructors fail, resulting in `Error::BadFileHandle`.
- **Insufficient Data**: If the stream ends prematurely, the underlying primitive parsers return an `IO` error, which propagates up.

Complexity:
- Time: O(N), where N is the total size of the variable-length fields (caller name, opaque handle, file handle).
- Space: O(N), where N is the size of the allocated strings and vectors within the returned structures.

Determinism:
- Deterministic. Given the same input byte sequence, the functions will always produce the same output structures or the same errors.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
 - `vector(src)`: Reads a length-prefixed variable byte array. Used for parsing the opaque lock owner handle.
 - `string_max_size(src, max)`: Reads a length-prefixed string, enforcing a maximum length. Used for parsing the caller name.
 - `i32(src)` / `u64(src)`: Reads signed and unsigned 64-bit integers. Used for system ID, offset, and length.
- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle(src)`: Parses an NFSv3 file handle. Used to extract the file identifier from the lock arguments.
- **From `nfs_mamont::nlm::lock`**:
 - `Nlm4Lock::new(...)`: Validates the lock arguments (specifically the caller name) and constructs the struct. Used to ensure the parsed data is semantically valid before returning it.
- **From `nfs_mamont::nlm`**:
 - `OpaqueHandle::new(...)`: Validates the size of the opaque handle bytes. Used to enforce protocol limits on the owner identifier.

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It acts as a factory for `Nlm4Lock` and `OpaqueHandle` (defined in `crate::nlm`) and a namespace for procedure-specific parsers.

Relations:
- **Aggregation**: The module aggregates the parsing logic for specific NLM procedures (`cancel`, `lock`, `test`, `unlock`) into a single namespace.

Global Invariants:
- The `parse_lock` function expects the input stream to contain fields in the exact order defined by the NLMv4 XDR specification: `caller_name`, `file_handle`, `opaque_handle`, `svid`, `offset`, `length`.
- The `opaque_handle` function enforces that the length of the handle does not exceed `OPAQUE_HANDLE_SIZE`.

## 5. Error Model

Error Types:
- **`Error`**: The error enum defined in `crate::parser` (re-exported from `crate::rpc`).

Error Propagation Strategy:
- Custom enum (`Result`). The module uses the `?` operator to propagate errors from primitive parsers (e.g., `IO` errors). It explicitly maps validation errors from `Nlm4Lock::new` and `OpaqueHandle::new` to `Error::BadFileHandle` using `map_err`.

Recoverability:
- Unrecoverable for the current parsing operation. If an error occurs, the stream cursor is likely at an undefined position relative to the message boundary, and the request cannot be retried without resetting the stream.

Panics:
- Allowed: No.
- Conditions: The code relies on fallible parsing functions and error mapping; there are no `unwrap` or `expect` calls in the public API.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to deserialize the common data structures shared by the Network Lock Manager (NLM) version 4 protocol procedures and to organize the parsers for specific NLM operations. The system contains an NFS server that implements file locking via the NLM protocol. This module serves as the foundational parsing layer for NLM, translating the raw XDR-encoded bytes received over the network into the Rust types (`Nlm4Lock`, `OpaqueHandle`) that the server's locking logic consumes.

A typical usage scenario of the system involves the RPC dispatcher receiving an NLM request (e.g., `LOCK`, `UNLOCK`). The dispatcher invokes the specific parser for that procedure (located in the submodules `lock`, `unlock`, etc.). These procedure-specific parsers rely on the `parse_lock` function from this module to extract the common lock details (caller name, file handle, lock range) which are present in almost every NLM call. Similarly, the `opaque_handle` function is used to extract the client-specific lock owner identifier.

Inside the system, the following things happen and they use this module:
- **Protocol Enforcement**: The module ensures that incoming data adheres to NLM limits (e.g., `LM_MAXSTRLEN` for hostnames) by validating the parsed structures immediately after reading them from the stream.
- **Code Reuse**: By delegating file handle parsing to `parser::nfsv3::file`, the module ensures that the server interprets file handles consistently between the NFS data protocol and the NLM locking protocol.
- **Namespace Organization**: The module groups the specific procedure parsers (`cancel`, `lock`, `test`, `unlock`) under a single path (`nfs_mamont::parser::nlm`), providing a clean interface for the RPC layer to dispatch NLM requests.