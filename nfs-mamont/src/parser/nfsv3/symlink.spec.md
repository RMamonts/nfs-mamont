<!-- SPEC_HASH: 62f0e1be794ea157d2f60cea3788ee2cae957542551ff90f4d72469dc82c604f -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::symlink
Rust File: src/parser/nfsv3/symlink.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the generic source trait for the `src` parameter in the `args` function. It allows the parser to consume bytes from network streams or test buffers.
- **`crate::parser::nfsv3::file`**: Used to import `handle`, `file_name`, and `file_path`. These functions are required to parse the directory file handle, the name of the symbolic link to be created, and the target path content of the link, respectively.
- **`crate::parser::nfsv3::set_attr`**: Used to import `new_attr`. This function is required to parse the `sattr3` structure, which allows the client to specify initial attributes (mode, uid, gid, size, timestamps) for the new symbolic link.
- **`crate::vfs::symlink`**: Used to import the `Args` structure. This is the target data structure that the parsing function populates and returns, representing the canonical arguments for the VFS `symlink` operation.
- **`crate::vfs`**: Used to import `DirOpArgs`. This structure is used to wrap the directory handle and the link name, forming the `object` field of `symlink::Args`.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that parsing errors are reported consistently with the rest of the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `SYMLINK` procedure from a byte stream into the high-level `vfs::symlink::Args` structure.
- To orchestrate the parsing of heterogeneous data types (handles, strings, attribute structures) into a single coherent argument object.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream (e.g., network socket or test cursor) from which XDR-encoded data is read.

Outputs:
- `Result<symlink::Args>`: The fully parsed arguments for the SYMLINK procedure.

Steps:
1. **Directory Context Parsing**: Calls `file::handle(src)` to read the file handle of the directory where the link will be created.
2. **Link Name Parsing**: Calls `file::file_name(src)` to read the name of the symbolic link.
3. **Directory Arguments Construction**: Constructs a `vfs::DirOpArgs` struct using the handle and name parsed in the previous steps.
4. **Attribute Parsing**: Calls `set_attr::new_attr(src)` to parse the optional attributes to be set on the new link.
5. **Target Path Parsing**: Calls `file::file_path(src)` to read the string content of the symbolic link (the path it points to).
6. **Result Construction**: Aggregates the `DirOpArgs`, attributes, and path into a `symlink::Args` struct and returns it wrapped in `Ok`.

Edge Cases:
- **Malformed Data**: If any of the sub-parsers (`handle`, `file_name`, `new_attr`, `file_path`) return an `Err` (e.g., due to invalid XDR encoding, length limits, or I/O errors), the `args` function propagates this error immediately, terminating the parsing process. This is verified by the `test_symlink_unaligned_path` test case.

Complexity:
- Time: O(N) where N is the total length of the variable-length fields (link name and target path). Fixed-size fields (handle, integers) are O(1).
- Space: O(N) for the heap-allocated strings within `file::Name` and `file::Path`.

Determinism:
- Deterministic (Given the same input byte stream, the function produces the same output or error).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`**: Provides the logic to read and validate the 64-byte file handle identifying the parent directory.
 - **`file_name`**: Provides the logic to read and validate the filename of the new link, enforcing length limits and UTF-8 validity.
 - **`file_path`**: Provides the logic to read and validate the target path string, which constitutes the content of the symbolic link.

- **From `nfs_mamont::parser::nfsv3::set_attr`**:
 - **`new_attr`**: Provides the logic to parse the `sattr3` structure. This is reused from the `create` module logic to handle the optional fields for mode, uid, gid, size, and timestamps.

- **From `nfs_mamont::vfs`**:
 - **`DirOpArgs`**: The structure used to encapsulate the directory handle and entry name, standardizing the input for directory modification operations.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a transformer, converting byte streams into the entities defined in `nfs_mamont::vfs::symlink`.

Relations:
- **Transformation**: The `args` function acts as an adapter between the `Read` stream and the `vfs::symlink::Args` struct.
- **Composition**: `symlink::Args` is composed of `vfs::DirOpArgs` (parsed via `file` functions), `set_attr::NewAttr` (parsed via `set_attr`), and `file::Path` (parsed via `file`).

Global Invariants:
- **Parsing Order**: The fields must be parsed in the specific order defined by the NFSv3 XDR specification for `SYMLINK3args`: directory handle, link name, attributes, and then link data.

## 5. Error Model

Error Types:
- **`parser::Error`**: The primary error type used throughout the module.
 - `Error::IO`: Propagated from the underlying `Read` stream or from dependency parsers if the stream ends unexpectedly.
 - `Error::BadFileHandle`: Propagated from `file::handle` if the handle size is incorrect.
 - `Error::MaxElemLimit`: Propagated from `file::file_name` or `file::file_path` if strings exceed protocol limits.
 - `Error::EnumDiscMismatch`: Propagated from `set_attr::new_attr` if attribute discriminants are invalid.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used extensively to propagate errors from `file` and `set_attr` parsers. If any step fails, the entire `args` function fails immediately.

Recoverability:
- **Unrecoverable for the current operation**: If parsing fails, the stream cursor is likely in an undefined state relative to the higher-level RPC message. The caller (typically the RPC dispatcher) must discard the current request.

Panics:
- Allowed: No
- Conditions: The code relies on `Result` propagation from dependencies; no `unwrap` or `expect` calls are present in the parsing logic.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **deserialize the arguments for the NFSv3 SYMLINK procedure** from the network wire format into the structured types required by the server's Virtual File System (VFS). The system contains a parser hierarchy where `primitive` handles raw bytes, `file` handles identifiers and strings, and `vfs::symlink` defines the semantic interface. This module sits in the middle, implementing the specific logic required to interpret the `SYMLINK3args` XDR structure.

A typical usage scenario of the system involves the RPC dispatcher receiving a `SYMLINK` request from an NFS client. The dispatcher identifies the procedure number and invokes the `args` function in this module. The function reads the directory handle and the new link name, then parses the initial attributes (mode, ownership, etc.) that the client wishes to set. Finally, it reads the target path string. The resulting `symlink::Args` struct is then passed to the VFS implementation to perform the actual symbolic link creation atomically.

Inside the system, the following things happen and they use this module:
- **Component Reuse**: The module imports `new_attr` from the `set_attr` module. This architectural choice ensures that the logic for parsing file attributes is consistent across `CREATE`, `MKDIR`, and `SYMLINK` operations, preventing code duplication.
- **Complex Argument Assembly**: The `SYMLINK` procedure is unique because it combines a directory operation (where to create the link), a file attribute set (metadata for the link), and a data payload (the target path). This module orchestrates these three distinct parsing concerns into a single function call.

Without this module, the RPC layer would lack a dedicated entry point for parsing `SYMLINK` requests. The logic would either be inlined into a generic dispatcher (making it complex and hard to maintain) or duplicated. This module ensures that the specific requirements of the `SYMLINK` procedure are correctly translated into the VFS layer's type-safe interface.