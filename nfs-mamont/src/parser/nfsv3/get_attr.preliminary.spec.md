<!-- SPEC_HASH: f099458ab983335613c76313712778cea1f35ced586114576f96f058816b6a6d -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::get_attr
Rust File: src/parser/nfsv3/get_attr.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the input source (`src`), allowing the parser to read bytes from any stream (e.g., network buffers, files).
- **`crate::parser::nfsv3::file`**: This module provides the `handle` function, which is responsible for the actual deserialization of the file handle bytes from the stream according to NFSv3/XDR standards.
- **`crate::vfs::get_attr`**: This module provides the `Args` structure, which serves as the target container for the parsed data. This structure is the standard input type for the VFS `GETATTR` operation.
- **`crate::parser`**: This module provides the `Result` type alias (presumably `Result<T, parser::Error>`), which is used for error propagation and unifying error handling across the parsing subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the specific argument payload for the NFSv3 `GETATTR` procedure from a byte stream.
- To encapsulate the parsing logic for this specific procedure, isolating the RPC dispatcher from the details of the wire format.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream. The stream position must be at the start of the `GETATTR` arguments (specifically, the file handle).

Outputs:
- `Result<get_attr::Args>`: 
 - `Ok(get_attr::Args)`: Contains the successfully parsed file handle.
 - `Err(parser::Error)`: Indicates a failure to read the stream or invalid data format (propagated from the underlying file handle parser).

Steps:
1. The function `args` is called with a reference to the input stream.
2. It delegates the parsing of the file handle to `crate::parser::nfsv3::file::handle(src)`.
3. If `file::handle` returns an `Ok` value containing a `file::Handle`, this handle is wrapped inside the `get_attr::Args` struct.
4. The `Ok` variant containing `get_attr::Args` is returned.
5. If `file::handle` returns an `Err`, that error is propagated immediately to the caller.

Edge Cases:
- **Stream Exhaustion**: If the stream ends before the file handle can be fully read, an `IO` error (wrapped in `parser::Error`) will be returned by the dependency `file::handle`.
- **Invalid Handle**: If the bytes read do not constitute a valid NFSv3 file handle (e.g., incorrect length), `file::handle` will return a specific error (e.g., `BadFileHandle`), which is propagated.

Complexity:
- Time: O(N), where N is the size of the file handle (fixed at 64 bytes in NFSv3).
- Space: O(1) for the logic, plus the space required to hold the `file::Handle` (fixed size).

Determinism:
- Deterministic. Given the same byte sequence in the stream, the function will always produce the same `Args` struct or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` function**: This is the core parsing mechanism used. It reads a length-prefixed byte array from the stream and validates that the length matches `NFS3_FHSIZE`. It returns a `file::Handle` or a `parser::Error`.
- **From `nfs_mamont::vfs::get_attr`**:
 - **`Args` struct**: This is the data transfer object. The module relies on this struct to define the expected output schema. It contains a single field `file: file::Handle`.

---

## 4. Data Model

Entities:
- **`get_attr::Args`**: A structure representing the parsed arguments for a `GETATTR` request. It contains a single field `file` of type `file::Handle`.

Relations:
- **Composition**: `get_attr::Args` composes `file::Handle`.

Global Invariants:
- The `Args` struct is only considered valid if it contains a `file::Handle` that was successfully validated by the `file::handle` parser (i.e., correct length and readable bytes).

## 5. Error Model

Error Types:
- **`parser::Error`**: This type is not defined in this module but is returned by the `file::handle` function. Based on the dependency specification, it likely includes variants like `BadFileHandle`, `IO`, etc.

Error Propagation Strategy:
- Propagation via `?` operator. The module does not generate new errors but acts as a pass-through for errors originating from the `file` parsing logic.

Recoverability:
- Non-recoverable for the specific parsing operation. If the arguments cannot be parsed, the request cannot be processed by the VFS layer.

Panics:
- Allowed: No.
- Conditions: The code consists of a single call chain using the `?` operator and struct construction, neither of which panics under standard conditions.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines a free-standing function `args`.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to perform the deserialization of the `GETATTR` procedure arguments within the NFSv3 protocol parser. The system contains a multi-layered architecture where raw network bytes are first interpreted by the RPC layer, then by specific NFSv3 procedure parsers (like this one), and finally converted into Virtual File System (VFS) requests. 

A typical usage scenario of the system involves an NFS client sending a `GETATTR` request over the network. The RPC dispatcher receives the raw bytes and identifies the procedure number as `GETATTR`. It then invokes this module's `args` function, passing the remaining byte stream. The system needs this module to bridge the gap between the generic byte stream and the specific `vfs::get_attr::Args` structure required by the storage backend. Without this module, the higher-level logic would need to manually parse the file handle, violating separation of concerns and increasing the risk of protocol implementation errors.

Inside the system, the following things happen and they use this module:
- **Protocol Decoding**: The module delegates the complex task of reading and validating the file handle (checking length, reading bytes) to `parser::nfsv3::file::handle`. This ensures that the strict NFSv3 constraints on file handles are enforced consistently across all procedures.
- **Type Safety**: By returning a strongly-typed `get_attr::Args` struct, the module guarantees that the VFS layer receives exactly the data it expects—a valid `file::Handle`—and nothing else, preventing runtime errors associated with incorrect argument passing.