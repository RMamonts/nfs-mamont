<!-- SPEC_HASH: a1a629a35424a7cf87d0221dfdb469cf4ff70dc744c30ee040972d95f28c7967 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::fs_stat
Rust File: src/parser/nfsv3/fs_stat.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the trait bound for the `src` parameter in the `args` function. It allows the parser to consume bytes from any source that implements the standard read interface (e.g., network streams, byte buffers).
- **`crate::parser::nfsv3::file`**: Used to access the `handle` function. This function is responsible for the actual deserialization of the file handle bytes from the stream, including validation of the handle size against the NFSv3 specification.
- **`crate::vfs::fs_stat`**: Used to import the `Args` structure. This structure serves as the target type for the parsing operation, wrapping the parsed file handle into the domain model expected by the VFS layer.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that the function returns the canonical error type used throughout the parser subsystem, facilitating consistent error handling.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `FSSTAT` procedure from a byte stream into a structured `vfs::fs_stat::Args` object.
- To delegate the low-level byte reading and validation logic to the `file` module while providing a specific entry point for the `FSSTAT` procedure arguments.

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`. This represents the incoming network stream or buffer containing the XDR-encoded arguments.

Outputs:
- `Result<fs_stat::Args>`: A Result containing the parsed arguments on success, or a `parser::Error` on failure (e.g., if the stream ends prematurely or the handle is invalid).

Steps:
1. **Delegation**: The `args` function calls `file::handle(src)`. This reads the file handle length prefix and the subsequent opaque data bytes from the stream.
2. **Validation**: The `file::handle` function validates that the read length matches the expected `NFS3_FHSIZE` (64 bytes). If it does not, it returns an error which is propagated via the `?` operator.
3. **Construction**: If the handle is successfully parsed, it is assigned to the `root` field of a new `fs_stat::Args` struct.
4. **Return**: The `fs_stat::Args` struct is wrapped in `Ok` and returned to the caller.

Edge Cases:
- **Stream Exhaustion**: If `src` does not contain enough bytes to form a complete file handle, `file::handle` will return an `IO` error, which terminates the parsing immediately.
- **Invalid Handle Size**: If the length prefix in the stream indicates a size other than 64 bytes, `file::handle` returns `Error::BadFileHandle`, preventing the creation of an invalid `Args` struct.

Complexity:
- Time: O(1) effectively, as the file handle size is fixed at 64 bytes in NFSv3.
- Space: O(1) for the stack-allocated return struct.

Determinism:
- Deterministic. Given the same byte sequence in `src`, the function will always produce the same `Args` struct or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` function**: This is the primary mechanism used. It encapsulates the logic for reading an XDR opaque data structure (a length-prefixed byte array) and strictly enforcing the `NFS3_FHSIZE` constraint. The current module relies on this to ensure that the `root` field in `Args` is valid before passing it to the VFS layer.

- **From `nfs_mamont::vfs::fs_stat`**:
 - **`Args` struct**: This module acts as a factory for this struct. The struct defines the input contract for the `FSSTAT` operation in the VFS layer, requiring a `file::Handle` to identify the file system root being queried.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It produces an instance of `vfs::fs_stat::Args`.

Relations:
- **Transformation**: The `args` function transforms a byte stream (`src`) into a `vfs::fs_stat::Args` struct.

Global Invariants:
- **Handle Validity**: The `root` field of the returned `Args` is guaranteed to be a valid `file::Handle` (specifically, a 64-byte array) because the construction depends on the successful return of `file::handle`, which enforces this invariant.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. This includes errors propagated from `file::handle`, such as `Error::IO` (read failure) or `Error::BadFileHandle` (incorrect size).

Error Propagation Strategy:
- **Direct Propagation**: The module uses the `?` operator on the result of `file::handle(src)`. This immediately returns any error encountered during the reading of the file handle to the caller, without adding additional context or wrapping.

Recoverability:
- **Unrecoverable for Message**: If this function returns an error, the arguments for the `FSSTAT` procedure cannot be constructed. The RPC message is considered malformed, and the server typically aborts processing of that specific request.

Panics:
- Allowed: No
- Conditions: The code does not perform any operations that could panic (e.g., no `unwrap` or indexing on empty collections).

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This module is used in order to **parse the arguments for the NFSv3 `FSSTAT` procedure** from the network stream. The system contains a layered parser architecture where high-level procedure parsers (like this one) delegate the parsing of common data types to specialized utility modules. The `FSSTAT` procedure requires a single argument: a file handle identifying the file system root to query.

A typical usage scenario of the system involves the RPC dispatcher receiving a request message identified as an `FSSTAT` call. The dispatcher invokes the `args` function in this module, passing the network stream. The function reads the file handle bytes, validates them, and returns a `vfs::fs_stat::Args` struct. This struct is then passed to the VFS implementation to retrieve the file system's dynamic statistics (total space, free space, etc.).

Inside the system, the following things happen and they use this module:
- **Protocol Compliance**: By delegating to `file::handle`, the module ensures that the file handle adheres to the strict 64-byte size requirement of NFSv3. This prevents invalid or maliciously sized handles from propagating deeper into the server.
- **Type Safety**: The module bridges the gap between the raw byte stream and the type-safe `vfs::fs_stat::Args`. This allows the VFS layer to operate on strongly-typed Rust structs rather than raw byte arrays, reducing the risk of field misinterpretation.

Without this module, the RPC dispatcher would need to contain inline logic to parse `FSSTAT` arguments, or the `file` module would need to expose procedure-specific logic, violating separation of concerns. This module encapsulates the specific knowledge that "FSSTAT arguments consist of a single file handle."