<!-- SPEC_HASH: 8090f15e7d53e0f5bb059bff843f8a671a7ed497ffde66d3e5b2fd415bac389b -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::create
Rust File: src/parser/nfsv3/create.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the input source (`src`), allowing the parsing functions to consume bytes from any stream (e.g., network buffers, files).
- **`crate::parser::nfsv3::file`**: This module is used to parse the directory context of the creation operation. Specifically, `file::handle` is used to parse the directory file handle, and `file::file_name` is used to parse the name of the file to be created.
- **`crate::parser::primitive`**: This module provides the low-level XDR (External Data Representation) parsing primitives. It is used to read integers (`u32`, `u64`), optional fields (`option`), and fixed-size byte arrays (`array`) that constitute the wire format of the `CREATE` arguments.
- **`crate::vfs::create`**: This module defines the target data structures (`Args`, `How`, `Verifier`) that represent the high-level arguments for the VFS `Create` trait. The parser populates these structures.
- **`crate::vfs::set_attr`**: This module defines `NewAttr` and `SetTime`, which are used to represent the initial attributes that may be set during file creation (in `Unchecked` and `Guarded` modes).
- **`crate::vfs::file`**: This module defines the `Time` structure, which is used to represent timestamps within the `SetTime` enum.
- **`crate::consts::nfsv3::NFS3_CREATEVERFSIZE`**: This constant defines the size of the verifier array (8 bytes) used in the `Exclusive` creation mode to ensure atomic creation semantics.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the byte stream of an NFSv3 `CREATE` procedure call into the structured `create::Args` type used by the VFS layer.
- To interpret the `createhow3` union (represented as `create::How`) which dictates the parsing logic for the remainder of the arguments (attributes vs. verifier).

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded `CREATE3args` structure.

Outputs:
- `Result<T>`: A result containing the parsed structure `T` (e.g., `create::Args`, `create::How`, `set_attr::NewAttr`) or a `parser::Error` if the stream is invalid or contains unexpected discriminants.

Steps:
1. **Top-Level Parsing (`args`)**:
   - Parses the directory handle using `file::handle`.
   - Parses the filename using `file::file_name`.
   - Wraps these into `vfs::DirOpArgs`.
   - Parses the creation mode using `how`.
   - Constructs and returns `create::Args`.
2. **Creation Mode Parsing (`how`)**:
   - Reads a `u32` discriminant representing the creation mode.
   - If `0` (Unchecked): Parses `new_attr` and returns `create::How::Unchecked`.
   - If `1` (Guarded): Parses `new_attr` and returns `create::How::Guarded`.
   - If `2` (Exclusive): Reads a fixed-size byte array of size `NFS3_CREATEVERFSIZE` using `primitive::array`, wraps it in `create::Verifier`, and returns `create::How::Exclusive`.
   - For any other value: Returns `Error::EnumDiscMismatch`.
3. **Attribute Parsing (`new_attr`)**:
   - Parses optional `u32` values for `mode`, `uid`, and `gid` using `primitive::option`.
   - Parses an optional `u64` value for `size`.
   - Parses `set_time` for `atime` and `mtime`.
   - Constructs `set_attr::NewAttr`.
4. **Time Strategy Parsing (`set_time`)**:
   - Reads a `u32` discriminant.
   - If `0`: Returns `SetTime::DontChange`.
   - If `1`: Returns `SetTime::ToServer`.
   - If `2`: Parses `nfs_time` and returns `SetTime::ToClient`.
   - For any other value: Returns `Error::EnumDiscMismatch`.
5. **Timestamp Parsing (`nfs_time`)**:
   - Reads a `u32` for seconds.
   - Reads a `u32` for nanoseconds.
   - Constructs `file::Time`.

Edge Cases:
- **Invalid Discriminants**: If the `how` or `set_time` discriminant is not 0, 1, or 2, the function returns `Error::EnumDiscMismatch`.
- **Stream Exhaustion**: If the stream ends prematurely while reading required fields (e.g., the verifier in exclusive mode), the underlying `primitive` parsers will return an `IO` error, which propagates up.

Complexity:
- Time: O(1) for fixed-size fields (handles, integers, verifiers). O(N) for variable-length fields (filenames), where N is the length of the string, due to the dependency on `file::file_name`.
- Space: O(1) for the parsing logic itself, excluding the space allocated for the returned structures.

Determinism:
- Deterministic. Given the same input byte stream, the functions will always produce the same output structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
 - `option<T>`: Used to parse optional fields in `NewAttr`. It reads a boolean flag; if true, it invokes the closure to read the value, otherwise returns `None`.
 - `array<const N>`: Used to read the fixed-size verifier in `Exclusive` mode. It reads exactly `N` bytes from the stream.
 - `u32`, `u64`: Used to read integer discriminants and attribute values.
- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle`: Parses the directory file handle, validating that it matches the expected NFSv3 size.
 - `file_name`: Parses the filename string, enforcing length limits and UTF-8 validity.
- **From `nfs_mamont::vfs::create`**:
 - `How` enum: Defines the variants (`Unchecked`, `Guarded`, `Exclusive`) that determine the control flow of the `how` parser function.
 - `Verifier`: Wraps the byte array read during exclusive creation.
- **From `nfs_mamont::vfs::set_attr`**:
 - `NewAttr`: The target structure populated by the `new_attr` function.
 - `SetTime`: The target enum populated by the `set_time` function.

---

## 4. Data Model

Entities:
- **`create::Args`**: The top-level structure containing the directory context (`DirOpArgs`) and the creation method (`How`).
- **`create::How`**: An enum representing the creation strategy.
 - `Unchecked(set_attr::NewAttr)`: Create the file, overwriting if it exists, with the specified attributes.
 - `Guarded(set_attr::NewAttr)`: Create the file only if it does not exist, with the specified attributes.
 - `Exclusive(create::Verifier)`: Create the file only if it does not exist, using the verifier to guarantee atomicity.
- **`set_attr::NewAttr`**: A structure containing optional fields for `mode`, `uid`, `gid`, `size`, and mandatory `SetTime` fields for `atime` and `mtime`.
- **`set_attr::SetTime`**: An enum representing how to set timestamps (`DontChange`, `ToServer`, `ToClient`).
- **`file::Time`**: A structure containing `seconds` and `nanos`.

Relations:
- **Composition**: `create::Args` contains `create::How`.
- **Composition**: `create::How` variants contain either `set_attr::NewAttr` or `create::Verifier`.
- **Composition**: `set_attr::NewAttr` contains `set_attr::SetTime`.

Global Invariants:
- The `how` discriminant must be 0, 1, or 2 to be valid.
- The `set_time` discriminant must be 0, 1, or 2 to be valid.
- The `Verifier` array must be exactly `NFS3_CREATEVERFSIZE` bytes long.

## 5. Error Model

Error Types:
- **`parser::Error`**:
 - `EnumDiscMismatch`: Returned when the discriminant for `how` or `set_time` is not 0, 1, or 2.
 - `IO(io::Error)`: Propagated from the underlying `Read` trait or primitive parsers if the stream is exhausted or fails.
 - Other errors (e.g., `MaxElemLimit`, `IncorrectString`) may be propagated from `file::file_name` or `primitive::option`.

Error Propagation Strategy:
- Custom enum (`Result<T>`). Errors are propagated using the `?` operator. The module explicitly constructs `Error::EnumDiscMismatch` for invalid protocol states.

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

This module is used in order to interpret the `CREATE` operation arguments sent by an NFSv3 client, translating the raw XDR byte stream into the structured arguments required by the Virtual File System (VFS) layer. The system contains a complex NFS server implementation where the VFS layer is abstracted from the wire format details. A typical usage scenario involves a client requesting to create a new file. The client specifies *how* the file should be created (e.g., "create it only if it doesn't exist" using `Guarded` mode, or "create it exclusively" using `Exclusive` mode with a verifier). This module parses that intent.

Inside the system, the following things happen and they use this module: The RPC dispatcher receives a `CREATE` request. It invokes the `args` function in this module. The function reads the directory handle and filename to identify *where* to create the file. Then, it reads the `how` discriminant. If the client requested `Exclusive` creation, the parser reads a specific 8-byte verifier. This verifier is crucial for the VFS implementation to detect race conditions (e.g., if another client created the file simultaneously). If the client requested `Unchecked` or `Guarded` creation, the parser reads the `NewAttr` structure, which allows the client to set initial permissions or ownership. Without this module, the VFS `Create` trait would not receive the necessary context (like the verifier or initial attributes) to perform the operation correctly according to the NFSv3 specification.