<!-- SPEC_HASH: 3ef41e3a8d1b078916a99b530ac6d35e24b032cdbea2e1c4daa1b7e807c97c12 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::link
Rust File: src/parser/nfsv3/link.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a bound for the `src` parameter, allowing the parser to read bytes from any source that implements the `Read` trait (e.g., network streams, byte buffers).
- **`crate::parser::nfsv3::file`**: This module is used to parse the specific components of the `LINK` arguments. Specifically, `file::handle` is called to parse the file handles for both the source file and the target directory, and `file_name` is called to parse the name of the new link.
- **`crate::vfs::link`**: This module provides the `link::Args` structure, which is the target type that the `args` function populates and returns. This structure represents the arguments in the Virtual File System domain.
- **`crate::vfs`**: This module provides the `vfs::DirOpArgs` structure, which is used to compose the target directory handle and the new link name into a single object within `link::Args`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the byte stream representing the arguments of an NFSv3 `LINK` procedure into the structured `vfs::link::Args` type.
- To orchestrate the parsing of multiple distinct fields (file handles and strings) in the specific order defined by the NFSv3 protocol.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded arguments for the `LINK` operation.

Outputs:
- `Result<link::Args>`: A Result containing the parsed `link::Args` structure on success, or a `parser::Error` on failure.

Steps:
1. The function calls `file::handle(src)` to parse the file handle of the existing file (the source of the link) from the stream.
2. The function calls `file::handle(src)` again to parse the file handle of the directory where the link will be created (the target directory) from the stream.
3. The function calls `file_name(src)` to parse the string representing the name of the new link from the stream.
4. The function constructs a `vfs::DirOpArgs` instance using the target directory handle and the link name parsed in steps 2 and 3.
5. The function constructs and returns a `vfs::link::Args` instance containing the source file handle (from step 1) and the `DirOpArgs` (from step 4).

Edge Cases:
- **Malformed Input**: If the stream contains invalid data (e.g., incorrect file handle size, invalid string encoding), the underlying calls to `file::handle` or `file_name` will return an error, which is propagated immediately via the `?` operator.
- **Unexpected EOF**: If the stream ends prematurely before all fields can be read, an `IO` error will be raised by the underlying read operations.

Complexity:
- Time: O(1) relative to the logic flow, though dependent on the size of the fixed-length file handles and the variable-length filename string.
- Space: O(N) where N is the length of the filename string, as the string is allocated during parsing.

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `link::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle(src)`: Reads a fixed-size byte array (defined by `NFS3_FHSIZE`) from the stream and wraps it in a `file::Handle`. It validates that the length prefix matches the expected size.
 - `file_name(src)`: Reads a length-prefixed string from the stream, validates its length against `MAX_NAME_LEN`, and wraps it in a `file::Name`.

- **From `nfs_mamont::vfs::link`**:
 - `Args`: The structure that holds the parsed arguments. It consists of a source `file::Handle` and a target `vfs::DirOpArgs`.

- **From `nfs_mamont::vfs`**:
 - `DirOpArgs`: A structure that aggregates a directory `file::Handle` and a `file::Name`, representing a specific entry location within a directory.

---

## 4. Data Model

Entities:
- **Input Stream**: A sequence of bytes conforming to the NFSv3 XDR definition for `LINK3args`.
- **Parsed Arguments (`link::Args`)**: The output structure containing:
 - `file`: `file::Handle` - The handle of the existing file to be linked.
 - `link`: `vfs::DirOpArgs` - The target directory and the new name for the link.

Relations:
- **Transformation**: The `args` function transforms the linear byte stream into a hierarchical object structure (`Args` -> `DirOpArgs` -> `Handle`/`Name`).

Global Invariants:
- The order of fields in the byte stream must be: source file handle, target directory file handle, link name.
- The file handles must be exactly `NFS3_FHSIZE` bytes long (enforced by `file::handle`).
- The link name must not exceed `MAX_NAME_LEN` (enforced by `file_name`).

## 5. Error Model

Error Types:
- **`parser::Error`**: This type is re-exported or used via the `Result` type alias. Specific variants likely include:
 - `IO`: For read failures.
 - `BadFileHandle`: If the file handle length is incorrect.
 - `MaxElemLimit`: If the filename is too long.

Error Propagation Strategy:
- Propagation via `Result`. The function uses the `?` operator to forward errors returned by `file::handle` and `file_name` directly to the caller.

Recoverability:
- Non-recoverable for the current parsing operation. If an error occurs, the stream position is likely undefined or advanced partially, and the caller should abort processing the current request.

Panics:
- Allowed: No.
- Conditions: The code does not perform any unwrapping or operations that could panic. All potential failures are captured in the `Result`.

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

This module is used in order to deserialize the arguments for the NFSv3 `LINK` procedure from the network wire format into the internal VFS domain model. The system contains a parser layer that sits between the raw network transport and the high-level Virtual File System logic. A typical usage scenario involves the server receiving an RPC request containing a byte payload representing a request to create a hard link. The system must interpret these bytes to identify which file to link and where to create the link.

Inside the system, the following things happen and they use this module: The RPC dispatcher identifies the procedure as `LINK` and invokes the `args` function defined in this module. This function relies on `parser::nfsv3::file` to handle the low-level extraction of file handles and strings, ensuring they conform to protocol limits (like `NFS3_FHSIZE` and `MAX_NAME_LEN`). Once parsed, the resulting `vfs::link::Args` structure is passed to the VFS implementation (specifically the `Link` trait). This separation allows the VFS to operate on typed, validated Rust structures without concerning itself with the intricacies of XDR encoding or network byte order. Without this module, the system would lack a dedicated mechanism to translate `LINK` arguments, leading to a mixing of protocol parsing logic with file system operation logic.