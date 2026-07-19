<!-- SPEC_HASH: dbc8ec2522fad4c1eaafbdc0a4d8d4817007ba4ad9160987052fff11ddddc2db -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::commit
Rust File: src/parser/nfsv3/commit.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the `src` parameter, allowing the parsing function to consume bytes from any source that implements the standard reading interface (e.g., network streams or memory buffers).
- **`crate::parser::nfsv3::file`**: This module is used to parse the file handle field within the `COMMIT` arguments. Specifically, the `handle` function is invoked to deserialize the opaque file identifier into a `file::Handle` structure.
- **`crate::parser::primitive`**: This module is used to parse the integer fields of the `COMMIT` arguments. The `u64` function is used for the offset, and the `u32` function is used for the count. These functions handle the XDR (External Data Representation) standard requirements (Big-Endian encoding).
- **`crate::vfs::commit`**: This module provides the `Args` structure which serves as the output type for this parser. The parser populates the `file`, `offset`, and `count` fields of this structure.
- **`crate::parser`**: This parent module provides the `Result` type alias used for error handling throughout the parsing logic.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the byte stream representation of an NFSv3 `COMMIT` procedure call into a structured `commit::Args` object suitable for use by the Virtual File System (VFS) layer.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream. The stream position must be at the start of the `COMMIT` arguments structure.

Outputs:
- `Result<commit::Args>`: A Result containing the parsed `commit::Args` structure on success, or a `parser::Error` on failure.

Steps:
1. **File Handle Parsing**: The function calls `file::handle(src)` to read the file handle from the stream. This consumes the bytes corresponding to the file identifier and validates them according to NFSv3 standards.
2. **Offset Parsing**: The function calls `primitive::u64(src)` to read the next 8 bytes as a 64-bit unsigned integer, representing the offset in the file where the commit operation begins.
3. **Count Parsing**: The function calls `primitive::u32(src)` to read the next 4 bytes as a 32-bit unsigned integer, representing the number of bytes to commit.
4. **Structure Construction**: The function constructs an instance of `commit::Args` using the values obtained in the previous steps and returns it wrapped in `Ok`.

Edge Cases:
- **Stream Exhaustion**: If the `src` stream ends before all required fields (file handle, offset, count) can be read, the underlying primitive parsers will return an `IO` error, which propagates through the `?` operator.
- **Invalid File Handle**: If the file handle data in the stream is malformed (e.g., incorrect length), the `file::handle` function will return a specific error (e.g., `BadFileHandle`), which propagates immediately.

Complexity:
- Time: O(1). The function reads a fixed number of bytes determined by the protocol (file handle size + 8 bytes offset + 4 bytes count).
- Space: O(1). The function allocates only the `commit::Args` struct and the internal `file::Handle` (which is fixed size).

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `commit::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle(src) -> Result<file::Handle>`: This mechanism reads a fixed-size file handle from the stream. It ensures that the handle conforms to `NFS3_FHSIZE` and maps the raw bytes to the `file::Handle` type. This is critical for identifying the target file of the commit operation.
- **From `nfs_mamont::parser::primitive`**:
 - `u64(src) -> Result<u64>`: This mechanism reads a Big-Endian 64-bit integer. It is used to extract the `offset` field, ensuring correct interpretation of the byte order defined by the NFS protocol.
 - `u32(src) -> Result<u32>`: This mechanism reads a Big-Endian 32-bit integer. It is used to extract the `count` field, defining the range of data to flush.

---

## 4. Data Model

Entities:
- **`commit::Args`**: The primary data structure returned by this module. Based on the dependency facts, it contains:
 - `file: file::Handle`: Identifies the file.
 - `offset: u64`: The starting byte offset for the commit.
 - `count: u32`: The number of bytes to commit.

Relations:
- **Aggregation**: `commit::Args` aggregates a `file::Handle` and two primitive integers (`u64`, `u32`).

Global Invariants:
- The input stream must contain at least `NFS3_FHSIZE + 8 + 4` bytes to successfully parse the arguments without encountering an Unexpected End of File error.
- The order of fields in the byte stream is strictly defined as: File Handle, Offset, Count.

---

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type is re-exported from the parent module. While specific variants are not defined in this file, the dependency chain implies the following possible errors:
 - `Error::IO`: Propagated from `Read` operations if the stream ends unexpectedly.
 - `Error::BadFileHandle`: Propagated from `file::handle` if the handle length is invalid.

Error Propagation Strategy:
- Custom enum (`Result<T>`). The module uses the `?` operator to propagate errors returned by `file::handle`, `u64`, and `u32` directly to the caller.

Recoverability:
- Generally unrecoverable for the current parsing operation. If parsing fails, the arguments cannot be constructed, and the calling RPC handler must abort processing the request.

Panics:
- Allowed: No.
- Conditions: The code consists solely of function calls and struct construction. It does not perform any unchecked operations, indexing, or unwrapping that could lead to a panic.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines a free-standing function and does not implement traits for any types.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to interpret the specific payload of an NFSv3 `COMMIT` request received over the network. The `COMMIT` operation is essential in NFS for ensuring data integrity, as it forces the server to write cached ("unstable") data to stable storage (disk). The system contains a layered parsing architecture where high-level RPC dispatchers identify the procedure type and delegate the deserialization of arguments to specific sub-modules. Without this module, the server would be unable to extract the file handle and byte range from the incoming request, making it impossible to execute the commit operation on the Virtual File System (VFS).

A typical usage scenario of the system involves a client sending an RPC request with a procedure number indicating `COMMIT`. The RPC dispatcher routes this to the NFSv3 handler, which calls this module's `args` function. The function reads the byte stream, extracting the file handle (to identify *what* to commit), the offset (to identify *where* to start), and the count (to identify *how much* to commit). The resulting `commit::Args` structure is then passed to the VFS trait implementation (specifically the `Commit` trait), which performs the actual I/O synchronization.

Inside the system, the following things happen and they use this module:
- **Protocol Compliance**: The module ensures that the `COMMIT` arguments are parsed according to the XDR standard and NFSv3 specification, correctly handling Big-Endian integers and the specific file handle format.
- **Abstraction**: It hides the complexity of byte-level manipulation (handled by `primitive` and `file` modules) from the upper layers of the server, providing a clean, type-safe API (`commit::Args`) that represents the logical intent of the client request.