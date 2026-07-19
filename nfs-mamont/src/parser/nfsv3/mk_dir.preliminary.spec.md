<!-- SPEC_HASH: 3dce288def7d104666824f909824d3c4567691d63e8acf71329413e43e6f64d9 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::mk_dir
Rust File: src/parser/nfsv3/mk_dir.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the input source (`src`), allowing the parsing function to consume bytes from any stream (e.g., network buffers, files).
- **`crate::parser::nfsv3::file`**: This module is used to parse the directory context of the `MKDIR` operation. Specifically, `file::handle` is used to parse the parent directory's file handle, and `file::file_name` is used to parse the name of the new directory.
- **`crate::parser::nfsv3::create`**: This module provides the `new_attr` function, which is reused here to parse the `sattr3` (set attributes) structure from the wire format. This allows the client to specify initial attributes for the new directory.
- **`crate::vfs::mk_dir`**: This module defines the target data structure `Args` that the parser populates. This structure is the input type for the VFS `MkDir` trait.
- **`crate::parser`**: This module provides the `Result` type alias, which is used to propagate parsing errors (e.g., `IO`, `BadFileHandle`, `MaxElemLimit`) to the caller.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the byte stream of an NFSv3 `MKDIR` procedure call into the structured `mk_dir::Args` type used by the VFS layer.
- To compose the parsed directory handle, name, and attributes into a single argument object representing the intent to create a directory.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded `MKDIR3args` structure.

Outputs:
- `Result<mk_dir::Args>`: A result containing the parsed arguments for the `MKDIR` operation or a `parser::Error` if the stream is invalid.

Steps:
1. **Parse Directory Handle**: Calls `file::handle(src)` to read the file handle of the parent directory where the new directory will be created.
2. **Parse Directory Name**: Calls `file::file_name(src)` to read the name of the new directory.
3. **Construct Directory Operation Arguments**: Wraps the parsed handle and name into `vfs::DirOpArgs`.
4. **Parse Attributes**: Calls `create::new_attr(src)` to read the optional initial attributes (mode, uid, gid, size, atime, mtime) for the new directory.
5. **Construct Final Arguments**: Combines the `vfs::DirOpArgs` and the parsed attributes into `mk_dir::Args` and returns it.

Edge Cases:
- **Stream Exhaustion**: If the stream ends prematurely while reading the handle, name, or attributes, the underlying parsers will return an `IO` error, which propagates up.
- **Invalid Data**: If the file handle size is incorrect or the filename is too long, the specific errors (`BadFileHandle`, `MaxElemLimit`) from the `file` module are propagated.

Complexity:
- Time: O(N), where N is the length of the directory name string. The handle and attributes are fixed-size or bounded by small constants.
- Space: O(N) for the allocated string representing the directory name. Other structures are stack-allocated or small heap allocations.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same output structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle`: Parses the directory file handle, validating that it matches the expected NFSv3 size (64 bytes).
 - `file_name`: Parses the filename string, enforcing length limits and UTF-8 validity.
- **From `nfs_mamont::parser::nfsv3::create`**:
 - `new_attr`: Parses the `sattr3` structure, which includes optional fields for mode, uid, gid, size, and specific strategies for setting atime and mtime. This mechanism is shared with the `CREATE` operation parser.
- **From `nfs_mamont::vfs::mk_dir`**:
 - `Args`: The target structure that aggregates the directory operation context (`DirOpArgs`) and the initial attributes (`NewAttr`).

---

## 4. Data Model

Entities:
- **`mk_dir::Args`**: The top-level structure produced by this module.
 - `object: vfs::DirOpArgs`: Identifies the parent directory and the new name.
 - `attr: set_attr::NewAttr`: Specifies the initial attributes for the new directory.

Relations:
- **Composition**: `mk_dir::Args` is composed of `vfs::DirOpArgs` and `set_attr::NewAttr`.
- **Dependency**: `vfs::DirOpArgs` is composed of `file::Handle` and `file::Name`.

Global Invariants:
- The byte stream `src` must conform to the XDR encoding of `MKDIR3args` as defined in RFC 1813.
- The order of parsing is strictly: directory handle, directory name, attributes.

## 5. Error Model

Error Types:
- **`parser::Error`**:
 - `BadFileHandle`: Propagated from `file::handle` if the handle size is invalid.
 - `MaxElemLimit`: Propagated from `file::file_name` if the name is too long.
 - `IO(io::Error)`: Propagated if the stream ends unexpectedly or fails to read.
 - Other errors (e.g., `EnumDiscMismatch`) may be propagated from `create::new_attr` if time attributes are invalid.

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

This module is used in order to interpret the `MKDIR` operation arguments sent by an NFSv3 client, translating the raw XDR byte stream into the structured arguments required by the Virtual File System (VFS) layer. The system contains a complex NFS server implementation where the VFS layer is abstracted from the wire format details. A typical usage scenario involves a client requesting to create a new directory. The client specifies the parent directory (via a file handle), the name of the new directory, and optionally, the initial attributes (permissions, ownership) for that directory.

Inside the system, the following things happen and they use this module: The RPC dispatcher receives a `MKDIR` request. It invokes the `args` function in this module. The function reads the directory handle and filename to identify *where* and *what* to create. Then, it reads the attribute data. By reusing the `new_attr` function from the `create` module, the system ensures that attribute parsing logic is consistent between creating files and creating directories. The resulting `mk_dir::Args` structure is then passed to the VFS `MkDir` trait implementation. Without this module, the VFS would not receive the necessary context (parent handle, name, and initial attributes) to perform the operation correctly according to the NFSv3 specification.