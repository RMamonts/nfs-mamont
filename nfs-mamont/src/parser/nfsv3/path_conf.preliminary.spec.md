<!-- SPEC_HASH: 9094b2afe2c0812dee8295fc4af7ea74b3171187ed10d682d0670a0e3b50f2b5 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::path_conf
Rust File: src/parser/nfsv3/path_conf.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the input source (`src`), allowing the parser to read bytes from any stream (e.g., network socket, memory buffer).
- **`crate::parser::nfsv3::file`**: This module is used to parse the file handle from the input stream. Specifically, the `handle` function is invoked to deserialize the XDR-encoded file handle bytes into a `file::Handle` structure, performing necessary validation (e.g., checking the length against `NFS3_FHSIZE`).
- **`crate::vfs::path_conf`**: This module provides the `Args` structure which serves as the output of this parsing function. The `Args` struct wraps the parsed file handle, representing the formal arguments required by the VFS layer to perform a `PATHCONF` operation.
- **`crate::parser`**: This module provides the `Result` type alias (and the underlying `Error` enum) used for error handling and propagation throughout the parsing logic.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments of an NFSv3 `PATHCONF` procedure call from a raw byte stream into a structured `path_conf::Args` object suitable for use by the VFS layer.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the serialized arguments of an NFSv3 `PATHCONF` request. According to the NFSv3 specification (RFC 1813), this consists of a single file handle.

Outputs:
- `Result<path_conf::Args>`: A result containing the parsed arguments structure or a `parser::Error` if the stream is malformed or incomplete.

Steps:
1. **File Handle Extraction**: The function calls `file::handle(src)` to read the file handle from the stream. This involves reading a length prefix followed by the handle bytes and validating the length.
2. **Argument Construction**: If the file handle is successfully parsed, it is wrapped in the `path_conf::Args` structure (`path_conf::Args { file: ... }`).
3. **Return**: The populated `Args` structure is returned wrapped in `Ok`. If `file::handle` returns an error, that error is propagated immediately.

Edge Cases:
- **Malformed File Handle**: If the file handle length prefix does not match the expected `NFS3_FHSIZE` (64 bytes), `file::handle` will return `Error::BadFileHandle`, which propagates out of this function.
- **Unexpected End of Stream**: If the stream ends before the file handle can be fully read, an `IO` error is propagated.

Complexity:
- Time: O(1) regarding logic flow, though it depends on the fixed-size read operation of the file handle (64 bytes).
- Space: O(1) additional space (allocating the `Args` struct and the `Handle` array).

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle(src: &mut impl Read) -> Result<file::Handle>`**: This is the core parsing mechanism used. It reads a length-prefixed byte array from the stream and strictly validates that the length matches `NFS3_FHSIZE`. It ensures that the `file` field in the resulting `Args` is a valid, protocol-compliant file handle before the `Args` struct is constructed.

---

## 4. Data Model

Entities:
- **`path_conf::Args`**: The output structure representing the deserialized arguments. It contains a single field `file` of type `file::Handle`.
- **Input Stream**: The source of raw bytes representing the XDR-encoded arguments.

Relations:
- **Composition**: `path_conf::Args` directly owns a `file::Handle`.

Global Invariants:
- The input stream must contain exactly one file handle encoded in XDR format (length followed by bytes).
- The `file` field within the returned `Args` is guaranteed to be a valid `file::Handle` (i.e., length == 64) if the function returns `Ok`.

## 5. Error Model

Error Types:
- **`parser::Error`**: The specific error types depend on the `file::handle` function, primarily:
 - `BadFileHandle`: If the length prefix is incorrect.
 - `IO(std::io::Error)`: If the read operation fails.

Error Propagation Strategy:
- Propagation using the `?` operator. The function does not introduce new error types but passes through errors from the underlying `file::handle` parser.

Recoverability:
- Non-recoverable for the specific parsing operation. If parsing fails, the stream cannot be processed further in the context of this specific RPC request.

Panics:
- Allowed: No.
- Conditions: The code uses `?` for error propagation and does not call `unwrap`, `expect`, or `panic`.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines a free-standing parsing function.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to deserialize the specific arguments required for the NFSv3 `PATHCONF` procedure from the network byte stream into a structured format that the Virtual File System (VFS) can understand. The system contains a complex network stack that receives RPC requests as opaque byte arrays. A typical usage scenario involves an RPC dispatcher receiving a `PATHCONF` request, which asks the server to return information about file system limits (like maximum filename length) for a specific file or directory. The system uses this module to interpret the raw bytes of that request. Specifically, it extracts the file handle (which identifies the target object) from the stream using the `file::handle` parser and packages it into the `vfs::path_conf::Args` structure.

Inside the system, the following things happen and they use this module: The server receives a TCP packet containing an NFSv3 `PATHCONF` call. The generic RPC layer passes the payload bytes to this module's `args` function. This function reads the file handle bytes, validates them (ensuring they are exactly 64 bytes long as per NFSv3 spec), and constructs an `Args` struct. This struct is then passed to the VFS implementation (specifically to the `PathConf` trait's `path_conf` method). Without this module, the VFS would not receive the correctly typed and validated input needed to look up the file system properties requested by the client. This module acts as the adapter between the wire-format protocol (XDR) and the internal domain model (VFS).