<!-- SPEC_HASH: a1a629a35424a7cf87d0221dfdb469cf4ff70dc744c30ee040972d95f28c7967 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::fs_stat
Rust File: src/parser/nfsv3/fs_stat.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the `src` parameter, allowing the parser to consume bytes from any source that implements the `Read` interface (e.g., network streams, byte buffers).
- **`crate::parser::nfsv3::file`**: This module provides the `handle` function, which is responsible for the actual deserialization and validation of the file handle from the byte stream. The `fs_stat` module delegates this specific parsing task to it.
- **`crate::vfs::fs_stat`**: This module provides the `Args` structure, which serves as the target data type for the parsing operation. The `args` function populates this structure to be passed to the VFS layer.
- **`crate::parser`**: This module provides the `Result` type alias (presumably `Result<T, Error>`), which is used for error handling and propagation throughout the parsing logic.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `FSSTAT` procedure from a byte stream into a structured `fs_stat::Args` object.
- To delegate the specific parsing of the file handle to the specialized `file` module, ensuring that validation logic (like checking file handle size) is centralized.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream. The stream position must be at the start of the `FSSTAT` arguments structure.

Outputs:
- `Result<fs_stat::Args>`: A result containing the parsed `Args` structure on success, or a `parser::Error` on failure (propagated from the underlying file handle parser).

Steps:
1. **Delegate Parsing**: The `args` function calls `file::handle(src)`, passing the input stream.
2. **Handle Extraction**: The `file::handle` function reads a length prefix (u32) from the stream, validates it against `NFS3_FHSIZE` (64 bytes), and reads the subsequent bytes into a `file::Handle` structure.
3. **Structure Construction**: The parsed `file::Handle` is wrapped in the `fs_stat::Args` struct: `fs_stat::Args { root: handle }`.
4. **Return**: The populated `Args` struct is returned wrapped in `Ok`.

Edge Cases:
- **Invalid Handle Length**: If the length prefix read by `file::handle` does not match `NFS3_FHSIZE`, the function returns `Error::BadFileHandle`.
- **Unexpected EOF**: If the stream ends before the full handle (length + bytes) can be read, an `IO` error is returned.

Complexity:
- Time: O(1) (specifically, O(68) bytes read: 4 for length + 64 for handle).
- Space: O(1) (allocates the fixed-size `Args` struct and `Handle`).

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `Args` struct or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` function**: This is the core mechanism used. It reads a big-endian u32 length prefix, strictly checks if it equals `NFS3_FHSIZE` (64), and then reads the handle bytes. It returns `Error::BadFileHandle` if the size is incorrect, ensuring protocol compliance.
- **From `nfs_mamont::vfs::fs_stat`**:
 - **`Args` struct**: This is the data container. It holds a single field `root` of type `file::Handle`, representing the file system root to be queried.

---

## 4. Data Model

Entities:
- **Input Stream**: A sequence of bytes representing the XDR-encoded arguments.
- **`fs_stat::Args`**: The output structure containing a `root` field.
- **`file::Handle`**: The inner structure representing the opaque file handle bytes.

Relations:
- **Composition**: `fs_stat::Args` owns one `file::Handle` instance.

Global Invariants:
- The input stream must contain a valid file handle encoding (length + data) at the current cursor position.
- The file handle length must be exactly 64 bytes (`NFS3_FHSIZE`) for the parsing to succeed.

## 5. Error Model

Error Types:
- **`parser::Error`** (propagated from `file::handle`):
 - `BadFileHandle`: Indicates the length prefix in the stream did not match the expected 64 bytes.
 - `IO(std::io::Error)`: Indicates a failure to read the required bytes from the source.

Error Propagation Strategy:
- Propagation via the `?` operator. The `args` function does not implement its own error mapping; it directly returns the result of `file::handle`.

Recoverability:
- Non-recoverable for the current parsing operation. If the handle cannot be parsed, the `Args` cannot be constructed, and the calling RPC handler must abort processing the request.

Panics:
- Allowed: No.
- Conditions: The code uses the `?` operator and does not call `unwrap`, `expect`, or `panic!`.

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

This module is used in order to parse the specific arguments required by the NFSv3 `FSSTAT` procedure, translating the raw XDR wire format into the `vfs::fs_stat::Args` structure used by the internal file system abstraction. The system contains a high-performance NFS server that handles multiple procedure types, each with unique argument structures. A typical usage scenario involves the RPC dispatcher receiving a `FSSTAT` request packet from a client. The dispatcher needs to extract the file handle (identifying the file system or mount point being queried) from the packet payload to pass it to the VFS layer.

Inside the system, the following things happen and they use this module: The dispatcher calls the `args` function defined here. This function acts as a specialized adapter that delegates the low-level byte reading and validation to the `file::handle` function. The `file` module ensures that the handle strictly adheres to the NFSv3 specification (specifically the 64-byte size limit), preventing malformed data from propagating deeper into the system. Once validated, the handle is wrapped in `fs_stat::Args`. This structured argument is then passed to the `Vfs::fs_stat` trait method. Without this module, the VFS layer would have to handle raw byte parsing directly, violating separation of concerns and mixing protocol logic with file system logic. By isolating the parsing logic here, the system ensures that the VFS implementation remains agnostic to the wire format details of the NFS protocol.