<!-- SPEC_HASH: a03e0ba0170702a5d26ac51421abd42f926614abd563ee39bb8826ce6450f05f -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::fs_info
Rust File: src/parser/nfsv3/fs_info.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the `src` parameter in the `args` function. It allows the parser to read bytes from any source that implements the `Read` trait (e.g., network streams, byte buffers).
- **`crate::parser::nfsv3::file`**: This module is used to parse the file handle from the input stream. Specifically, the `handle` function is called to deserialize the opaque file handle bytes into a `file::Handle` structure.
- **`crate::vfs::fs_info`**: This module provides the `fs_info::Args` structure which is the return type of the `args` function. This structure encapsulates the parsed arguments required for an NFSv3 `FSINFO` operation.
- **`crate::parser`**: This module provides the `Result` type alias (likely `Result<T, parser::Error>`) used for error handling and propagation within the parsing logic.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments of an NFSv3 `FSINFO` procedure call from a byte stream into a structured `fs_info::Args` object.
- To delegate the low-level parsing of the file handle to the specialized `file` module while constructing the high-level argument container.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded arguments for the `FSINFO` procedure.

Outputs:
- `Result<fs_info::Args>`: A Result containing the parsed `fs_info::Args` structure on success, or a `parser::Error` on failure.

Steps:
1. **File Handle Parsing**: The function calls `file::handle(src)` to read the file handle from the beginning of the stream. This operation consumes the bytes corresponding to the handle from `src`.
2. **Argument Construction**: If the file handle is parsed successfully, the function constructs an instance of `fs_info::Args`, setting its `root` field to the parsed handle.
3. **Return**: The constructed `Args` object is wrapped in `Ok` and returned. If `file::handle` returns an error, the error is propagated immediately using the `?` operator.

Edge Cases:
- **Stream Exhaustion**: If the `src` stream ends before the file handle can be fully read, an `IO` error (wrapped in `parser::Error`) will be returned by `file::handle` and propagated.
- **Invalid Handle**: If the file handle data in the stream is malformed (e.g., incorrect length according to NFSv3 specs), `file::handle` will return a specific error (e.g., `BadFileHandle`), which will be propagated.

Complexity:
- Time: O(N), where N is the size of the file handle being parsed (typically 64 bytes in NFSv3).
- Space: O(N), to store the parsed file handle within the `Args` structure.

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` function**: This function reads a length-prefixed opaque data structure from the stream. It validates that the length matches the expected `NFS3_FHSIZE` (64 bytes) and returns a `file::Handle` or an error. This is the core parsing logic utilized by the `args` function.

- **From `nfs_mamont::vfs::fs_info`**:
 - **`Args` struct**: This structure serves as the container for the parsed data. It consists of a single field `root` of type `file::Handle`, representing the file system root or mount point being queried.

---

## 4. Data Model

Entities:
- **`fs_info::Args`**: A structure defined in the `vfs` module but populated here. It contains a single field `root` which holds the file handle.

Relations:
- **Dependency**: The `args` function maps a byte stream (`src`) to an `fs_info::Args` entity.
- **Composition**: `fs_info::Args` composes a `file::Handle`.

Global Invariants:
- The `root` field of the returned `fs_info::Args` is guaranteed to be a valid `file::Handle` if the function returns `Ok`.

## 5. Error Model

Error Types:
- **`parser::Error`**: This enumeration (defined in the `crate::parser` module) represents various parsing failures. While this module does not define the error, it propagates errors originating from `file::handle`, which may include variants like `BadFileHandle`, `IO`, or `MaxElemLimit`.

Error Propagation Strategy:
- Propagation via the `?` operator. The `args` function does not introduce new error types but passes through any error encountered during the parsing of the file handle.

Recoverability:
- Non-recoverable for the specific parsing operation. If the arguments cannot be parsed, the request is considered malformed, and the calling RPC handler should typically abort processing.

Panics:
- Allowed: No.
- Conditions: The code consists of a single function call and struct construction. It does not explicitly panic.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines a single free-standing function and does not implement traits for any types.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to parse the incoming arguments for the NFSv3 `FSINFO` procedure, allowing the server to interpret client requests regarding file system capabilities. The system contains a complex parser subsystem that converts raw XDR (External Data Representation) byte streams into high-level Rust structures used by the Virtual File System (VFS). A typical usage scenario involves an NFS client sending an `FSINFO` RPC request immediately after mounting to discover server parameters like maximum read/write sizes or supported link types.

Inside the system, the following things happen and they use this module: The RPC dispatcher receives a raw byte buffer containing the `FSINFO` arguments. It invokes the `args` function defined in this module, passing the byte stream. The `args` function delegates the extraction of the file handle to the `file::handle` function (from the `parser::nfsv3::file` module). Once the file handle is successfully extracted and wrapped into `fs_info::Args`, this structure is passed to the VFS implementation (via the `FsInfo` trait). The VFS then uses the handle to identify the specific file system and returns its static properties. Without this module, the server would lack the specific logic to deserialize `FSINFO` requests, preventing clients from optimizing their interactions with the server or understanding the file system's limitations.