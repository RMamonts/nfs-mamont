<!-- SPEC_HASH: ea8cd6df295f3bd2217717734cb17809268cb7eabc6170379fa0bc2ffd5ef1f3 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::set_attr
Rust File: src/parser/nfsv3/set_attr.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the input source (`src`), allowing the parsing functions to consume bytes from any stream (e.g., network buffers).
- **`crate::parser::nfsv3::create`**: This module is used to parse the `new_attr` structure and `nfs_time`. The `SETATTR` and `CREATE` operations in NFSv3 share the `sattr3` (set attributes) structure definition in the wire format. By re-exporting and using `create::new_attr`, this module avoids duplicating the parsing logic for optional fields (mode, uid, gid, size) and timestamp strategies (`SetTime`).
- **`crate::parser::nfsv3::file`**: This module is used to parse the file handle (`file::handle`) that identifies the target object for the attribute modification.
- **`crate::parser::primitive`**: This module provides the `bool` function, which is used to parse the boolean flag indicating the presence of the `guard` field in the wire format.
- **`crate::vfs::set_attr`**: This module defines the target data structures (`Args`, `Guard`) that the parsing functions populate. These structures are the input types for the VFS `SetAttr` trait.
- **`crate::parser`**: This module provides the `Result` type alias and the `Error` enum used for error propagation throughout the parsing logic.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the byte stream of an NFSv3 `SETATTR` procedure call into the structured `set_attr::Args` type used by the VFS layer.
- To interpret the optional synchronization `guard` which allows the client to verify that the object has not been modified by another client since the last check (optimistic locking).

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded `SETATTR3args` structure.

Outputs:
- `Result<set_attr::Args>`: A result containing the parsed arguments for the `SETATTR` operation.
- `Result<Option<set_attr::Guard>>`: A result containing an optional guard structure.

Steps:
1. **Top-Level Parsing (`args`)**:
 - Invokes `file::handle(src)` to parse the file handle of the target object.
 - Invokes `create::new_attr(src)` to parse the `NewAttr` structure containing the attributes to modify.
 - Invokes the local `guard(src)` function to parse the optional synchronization guard.
 - Constructs and returns `set_attr::Args { file, new_attr, guard }`.
2. **Guard Parsing (`guard`)**:
 - Invokes `primitive::bool(src)` to read a boolean flag from the stream.
 - If the flag is `true`:
 - Invokes `create::nfs_time(src)` to parse the `ctime` (change time).
 - Returns `Ok(Some(Guard { ctime }))`.
 - If the flag is `false`:
 - Returns `Ok(None)`.

Edge Cases:
- **Stream Exhaustion**: If the stream ends prematurely while reading the file handle, attributes, or guard, the underlying parsers will return an `IO` error, which propagates up.
- **Invalid Boolean**: If the boolean flag for the guard is not 0 or 1, `primitive::bool` will return `Error::EnumDiscMismatch`.

Complexity:
- Time: O(1) for fixed-size fields (handles, integers, timestamps). The complexity is dominated by the dependency on `create::new_attr`, which parses optional fields but is effectively constant time relative to the stream size for this specific operation structure.
- Space: O(1) for the parsing logic itself, excluding the space allocated for the returned structures.

Determinism:
- Deterministic. Given the same input byte stream, the functions will always produce the same output structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::create`**:
 - `new_attr`: This function parses the `NewAttr` structure. It handles the logic for reading optional `u32` values (mode, uid, gid), an optional `u64` (size), and `SetTime` enums for atime and mtime. This is critical because `SETATTR` reuses the exact same attribute structure as `CREATE`.
 - `nfs_time`: This function parses a `file::Time` structure (seconds and nanoseconds), used within the `Guard` to represent the expected `ctime`.
- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle`: Parses the file handle, ensuring it conforms to the expected size (`NFS3_FHSIZE`).
- **From `nfs_mamont::parser::primitive`**:
 - `bool`: Reads a `u32` and maps it to a Rust `bool`, ensuring strict validation of the boolean value.

---

## 4. Data Model

Entities:
- **`set_attr::Args`**: The top-level structure containing the file handle, the new attributes to set, and an optional guard.
- **`set_attr::Guard`**: A structure containing `pub ctime: file::Time`. It acts as a verification token to ensure the object has not been modified by another client since the client last retrieved its attributes.

Relations:
- **Composition**: `set_attr::Args` aggregates `file::Handle`, `set_attr::NewAttr`, and `Option<set_attr::Guard>`.

Global Invariants:
- The byte stream `src` must be positioned such that the next bytes correspond to the `SETATTR3args` XDR structure.
- The `guard` field in the wire format is a boolean. If present, it is followed by an `nfs_time` structure.

## 5. Error Model

Error Types:
- **`parser::Error`**:
 - `IO(std::io::Error)`: Propagated from the underlying `Read` trait or primitive parsers if the stream is exhausted or fails.
 - `EnumDiscMismatch`: Propagated from `primitive::bool` if the guard flag is not 0 or 1, or potentially from `create::new_attr` if time discriminants are invalid.

Error Propagation Strategy:
- Custom enum (`Result<T>`). Errors are propagated using the `?` operator.

Recoverability:
- Generally non-recoverable for the specific RPC request. If the arguments cannot be parsed, the request cannot be processed, and the server should typically return an RPC `GarbageArgs` error.

Panics:
- Allowed: No.
- Conditions: The code does not explicitly panic. It relies on `Result` propagation for all failure modes.

---

## 6. Traits

List which external traits this module implements:
- None. This module only defines free-standing parsing functions.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to interpret the `SETATTR` operation arguments sent by an NFSv3 client, translating the raw XDR byte stream into the structured arguments required by the Virtual File System (VFS) layer. The system contains a complex NFS server implementation where the VFS layer is abstracted from the wire format details. A typical usage scenario involves a client requesting to modify file metadata, such as changing permissions (`mode`), truncating a file (`size`), or updating ownership (`uid`/`gid`). Additionally, the client may provide a `guard` containing the `ctime` (change time) of the file as it was when the client last read it. This module parses that guard.

Inside the system, the following things happen and they use this module: The RPC dispatcher receives a `SETATTR` request. It invokes the `args` function in this module. The function reads the file handle to identify the target object. It then reads the `new_attr` structure. Crucially, it reuses the `create::new_attr` function for this, as the NFSv3 protocol defines the "set attributes" structure identically for both creating and setting attributes. This allows the system to maintain a single source of truth for parsing these complex optional fields. Finally, it parses the optional `guard`. If the guard is present, the VFS implementation will use the parsed `ctime` to perform an optimistic locking check: if the file's current `ctime` differs from the one in the guard, the VFS will reject the operation with a `NotSync` error. Without this module, the VFS would lack the necessary parsed data to perform these metadata updates and synchronization checks securely.