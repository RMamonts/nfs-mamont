<!-- SPEC_HASH: 62f0e1be794ea157d2f60cea3788ee2cae957542551ff90f4d72469dc82c604f -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::symlink
Rust File: src/parser/nfsv3/symlink.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the input source (`src`), allowing the parsing functions to consume bytes from any stream (e.g., network buffers).
- **`crate::parser::nfsv3::file`**: This module provides the low-level parsing functions for the directory file handle (`handle`), the name of the symbolic link to be created (`file_name`), and the target path the link points to (`file_path`). It abstracts the XDR decoding details for these specific fields.
- **`crate::parser::nfsv3::set_attr`**: This module provides the `new_attr` function, which is used to parse the optional initial attributes (mode, uid, gid, size, timestamps) for the symbolic link. This reuses the attribute parsing logic common to other NFSv3 operations like `CREATE` and `SETATTR`.
- **`crate::vfs::symlink`**: This module defines the `symlink::Args` structure, which is the target output type of the `args` function. This structure aggregates the parsed data into a format compatible with the VFS layer's `Symlink` trait.
- **`crate::vfs`**: This module provides the `vfs::DirOpArgs` structure, which is used to group the directory handle and the new link name together within the `symlink::Args` output.
- **`crate::parser`**: This module provides the `Result` type alias (wrapping `parser::Error`), which is used for error propagation throughout the parsing logic.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the byte stream of an NFSv3 `SYMLINK` procedure call into the structured `symlink::Args` type used by the VFS layer.
- To validate the structure of the incoming request by ensuring all required fields (directory, name, attributes, path) are present and conform to XDR encoding rules.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded `SYMLINK3args` structure.

Outputs:
- `Result<symlink::Args>`: A result containing the parsed arguments for the `SYMLINK` operation, or a `parser::Error` if the stream is invalid or incomplete.

Steps:
1. **Directory Context Parsing**:
 - Invokes `file::handle(src)` to parse the file handle of the directory where the link will be created.
 - Invokes `file::file_name(src)` to parse the name of the symbolic link.
 - Wraps these two values into a `vfs::DirOpArgs` structure.
2. **Attribute Parsing**:
 - Invokes `set_attr::new_attr(src)` to parse the `sattr3` (set attributes) structure. This reads optional fields for mode, uid, gid, size, and timestamp strategies.
3. **Target Path Parsing**:
 - Invokes `file::file_path(src)` to parse the string representing the target path of the symbolic link.
4. **Result Construction**:
 - Constructs and returns `symlink::Args { object: DirOpArgs, attr: NewAttr, path: Path }`.

Edge Cases:
- **Malformed Stream**: If the byte stream does not contain valid XDR data (e.g., incorrect lengths, invalid UTF-8 in strings, or premature EOF), the underlying parsers (`file::handle`, `file::file_name`, etc.) will return an error, which propagates up.
- **Unaligned Data**: The test `test_symlink_unaligned_path` indicates that if the stream contains unexpected padding or garbage data following the path length field (or if the length is inconsistent with the remaining data), the parser returns an error.

Complexity:
- Time: O(N), where N is the total length of the filename and path strings. Parsing handles and fixed-size integers is O(1).
- Space: O(N) for the allocated strings (filename and path) within the returned `symlink::Args` structure.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same output structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle`: Parses the directory file handle, ensuring it strictly matches the `NFS3_FHSIZE` (64 bytes). If the size differs, it returns `Error::BadFileHandle`.
 - `file_name`: Parses the link name as a length-prefixed string. It enforces `MAX_NAME_LEN` and validates UTF-8 correctness.
 - `file_path`: Parses the target path as a length-prefixed string. It enforces `MAX_PATH_LEN` and validates UTF-8 correctness.
- **From `nfs_mamont::parser::nfsv3::set_attr`**:
 - `new_attr`: Parses the `NewAttr` structure. It handles the logic for reading optional `u32` values (mode, uid, gid), an optional `u64` (size), and `SetTime` enums for atime and mtime. This ensures the attribute data conforms to the `sattr3` XDR definition.

---

## 4. Data Model

Entities:
- **`symlink::Args`**: The top-level structure produced by this module. It contains:
 - `object: vfs::DirOpArgs`: The parent directory handle and the new link name.
 - `attr: set_attr::NewAttr`: The initial attributes to apply to the link.
 - `path: file::Path`: The target path the link points to.

Relations:
- **Composition**: `symlink::Args` aggregates `vfs::DirOpArgs`, `set_attr::NewAttr`, and `file::Path`.
- **Composition**: `vfs::DirOpArgs` aggregates `file::Handle` and `file::Name`.

Global Invariants:
- The byte stream `src` must be positioned such that the next bytes correspond to the `SYMLINK3args` XDR structure in the specific order: directory handle, filename, attributes, target path.

## 5. Error Model

Error Types:
- **`parser::Error`**:
 - `BadFileHandle`: Propagated from `file::handle` if the directory handle size is incorrect.
 - `MaxElemLimit`: Propagated from `file::file_name` or `file::file_path` if the strings exceed the maximum allowed length.
 - `IO(std::io::Error)`: Propagated if the stream ends prematurely or an I/O error occurs.
 - `EnumDiscMismatch`: Potentially propagated from `set_attr::new_attr` if timestamp discriminants are invalid.

Error Propagation Strategy:
- Custom enum (`Result<T>`). Errors are propagated using the `?` operator from the underlying parsing functions.

Recoverability:
- Generally non-recoverable for the specific RPC request. If the arguments cannot be parsed, the request cannot be processed, and the server should typically return an RPC `GarbageArgs` error.

Panics:
- Allowed: No.
- Conditions: The code does not explicitly panic. It relies on `Result` propagation for all failure modes.

---

## 6. Traits

List which external traits this module implements:
- None. This module only defines a free-standing parsing function.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to interpret the `SYMLINK` operation arguments sent by an NFSv3 client, translating the raw XDR byte stream into the structured arguments required by the Virtual File System (VFS) layer. The system contains a complex NFS server implementation where the VFS layer is abstracted from the wire format details. A typical usage scenario involves a client requesting to create a symbolic link at a specific path pointing to a target path. The client also provides optional attributes (like permissions) to set on the new link. This module parses that request.

Inside the system, the following things happen and they use this module: The RPC dispatcher receives a `SYMLINK` request. It invokes the `args` function in this module. The function reads the directory handle and filename to determine *where* to create the link. It then reads the `new_attr` structure. Crucially, it reuses the `set_attr::new_attr` function for this, as the NFSv3 protocol defines the "set attributes" structure identically for `SYMLINK`, `CREATE`, and `SETATTR` operations. This allows the system to maintain a single source of truth for parsing these complex optional fields. Finally, it parses the target path. Without this module, the VFS `Symlink` trait would not receive the necessary parsed data (directory context, attributes, and target path) to perform the operation correctly according to the NFSv3 specification. The module ensures that the data is strictly validated (e.g., handle sizes, string lengths) before it reaches the VFS, preventing malformed requests from affecting the file system state.