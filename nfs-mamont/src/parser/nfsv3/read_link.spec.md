<!-- SPEC_HASH: 75e9c7a048ceec5a86ebdf13780a3cd8a6cdbbdb76f629698559c51f344669b2 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::read_link
Rust File: src/parser/nfsv3/read_link.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in the `args` function. It allows the parser to read bytes from a generic stream (e.g., TCP stream or buffer).
- **`crate::parser::nfsv3::file`**: Used to access the `handle` function, which performs the low-level parsing of the file handle (an opaque byte array) from the input stream according to NFSv3 specifications.
- **`crate::vfs::read_link`**: Used to import the `read_link::Args` structure. This structure is the target type that the parser populates, serving as the input for the VFS layer's `ReadLink` operation.
- **`crate::parser`**: Used to import the `Result` type alias, which standardizes error handling across the parsing subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `READLINK` procedure from a byte stream into a structured `read_link::Args` object.
- To act as a specific adapter that maps the raw wire format (a file handle) to the VFS domain model.

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`, representing the stream of bytes containing the RPC arguments.

Outputs:
- `Result<read_link::Args>`: A Result containing the parsed arguments or a `parser::Error` if the stream is malformed or incomplete.

Steps:
1. **Handle Parsing**: The function calls `file::handle(src)` to read the file handle from the stream. This involves reading the length of the handle and then the handle data itself.
2. **Argument Construction**: The parsed `file::Handle` is wrapped in the `read_link::Args` struct: `read_link::Args { file: ... }`.
3. **Return**: The `Args` struct is wrapped in `Ok` and returned.

Edge Cases:
- **Malformed Handle**: If the file handle in the stream is invalid (e.g., incorrect length or I/O error), `file::handle` will return an error, which is propagated immediately via the `?` operator.

Complexity:
- **Time**: O(N) where N is the size of the file handle (fixed at 64 bytes in NFSv3, so effectively O(1)).
- **Space**: O(N) for the allocated `file::Handle` structure.

Determinism:
- **Deterministic**: Given the same input byte stream, the function will always produce the same `read_link::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` function**: This is the core parsing primitive used. It handles the XDR decoding of the file handle, including reading the length prefix and validating it against `NFS3_FHSIZE`. This module relies on it to ensure the handle is valid before wrapping it in `Args`.

- **From `nfs_mamont::vfs::read_link`**:
 - **`Args` struct**: This module acts as a constructor for this type. The `Args` struct defines the contract for the VFS layer, specifically requiring a `file::Handle` to identify the symbolic link to be read.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It constructs an instance of `read_link::Args` defined in the `vfs` module.

Relations:
- **Transformation**: The `args` function transforms a byte stream (`src`) into a `read_link::Args` struct.

Global Invariants:
- **Single Argument**: The `READLINK` procedure in NFSv3 takes exactly one argument: the file handle of the symbolic link. This module enforces this by only parsing one field.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. Specific errors likely include `IO` (if reading fails) or `BadFileHandle` (if the handle size is invalid), originating from the `file::handle` function.

Error Propagation Strategy:
- **Direct Propagation**: The module uses the `?` operator to forward errors from `file::handle` directly to the caller. It does not introduce new error types or perform error mapping.

Recoverability:
- **Unrecoverable for the Request**: If parsing fails, the RPC request cannot be processed further. The caller (typically the RPC dispatcher) must abort processing the request and return an error to the client.

Panics:
- **Allowed**: No.
- **Conditions**: The code consists of a single call and a struct construction, neither of which can panic under normal circumstances (assuming `Read` implementation is correct).

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

This module is used in order to **deserialize the arguments for the NFSv3 `READLINK` procedure**, bridging the gap between the raw network byte stream and the high-level Virtual File System (VFS) interface. The system contains a hierarchical parser architecture where generic RPC handling is separated from specific procedure logic. This module represents the leaf node for the `READLINK` operation, responsible for interpreting the specific binary layout expected by this procedure.

A typical usage scenario of the system involves the RPC dispatcher receiving a request message identified as a `READLINK` call. The dispatcher invokes this module's `args` function, passing the remaining bytes of the message. The function reads the file handle (the only argument for `READLINK`) and constructs a `read_link::Args` object. This object is then passed to the VFS backend, which implements the `ReadLink` trait to actually read the symbolic link's target from the storage backend.

Inside the system, the following things happen and they use this module:
- **Protocol Decoding**: The system relies on this module to correctly interpret the XDR (External Data Representation) format of the file handle. While the `file` module handles the byte-level reading, this module provides the semantic context that these bytes represent the target of a `READLINK` operation.
- **Type Safety**: By returning a strongly-typed `read_link::Args` struct instead of a generic byte array, this module ensures that the VFS layer receives exactly the data structure it expects (a `file::Handle`), preventing type errors at the boundary between the network parser and the service logic.

Without this module, the parser subsystem would lack a dedicated entry point for `READLINK` requests, forcing the dispatcher to use generic or ad-hoc parsing logic. This would violate the separation of concerns, mixing protocol dispatch logic with data structure deserialization. This module ensures that the specific parsing logic for `READLINK` is encapsulated and maintainable.