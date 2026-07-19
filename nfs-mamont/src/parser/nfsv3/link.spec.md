<!-- SPEC_HASH: 3ef41e3a8d1b078916a99b530ac6d35e24b032cdbea2e1c4daa1b7e807c97c12 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::link
Rust File: src/parser/nfsv3/link.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in the `args` function. It allows the parser to consume bytes from a generic stream (e.g., a network socket or a buffer).
- **`crate::parser::nfsv3::file`**: Used to access the `handle` and `file_name` functions. These are essential for deserializing the file handles (identifying the source file and target directory) and the new link name from the XDR wire format.
- **`crate::vfs::link`**: Used to import the `Args` structure. This is the target type that the `args` function constructs and returns, representing the parsed arguments in the domain model of the Virtual File System.
- **`crate::vfs`**: Used to import `DirOpArgs`. This structure is used to compose the target directory handle and the new link name into a single object, which is then embedded in `link::Args`.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that the function returns the canonical error type used throughout the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the specific binary payload of an NFSv3 `LINK` procedure call into a strongly-typed `link::Args` structure.
- To map the sequential XDR (External Data Representation) fields of the `LINK` arguments (source file, target directory, new name) to the composite VFS data structures.

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`. This represents the stream of bytes containing the encoded arguments.

Outputs:
- `Result<link::Args>`: A Result containing the parsed arguments (`link::Args`) or a `parser::Error` if deserialization fails.

Steps:
1. **Parse Source File**: The function calls `file::handle(src)` to read the file handle of the existing file to be linked. This advances the stream cursor by the size of the handle (typically 64 bytes).
2. **Parse Target Directory**: The function calls `file::handle(src)` again to read the file handle of the directory where the link will be created.
3. **Parse Link Name**: The function calls `file_name(src)` to read the variable-length string representing the name of the new link.
4. **Construct Directory Arguments**: The parsed directory handle and link name are combined into a `vfs::DirOpArgs` struct.
5. **Construct Final Arguments**: The source file handle and the `DirOpArgs` are combined into the final `link::Args` struct, which is then wrapped in `Ok` and returned.

Edge Cases:
- **Stream Exhaustion**: If `src` ends prematurely before all fields are read, the underlying `file::handle` or `file_name` functions will return an `IO` error, which propagates through the `?` operator.
- **Invalid Data**: If the file handle size is incorrect or the link name contains invalid characters or exceeds length limits, the helper functions from `file` will return specific parser errors (e.g., `BadFileHandle`, `MaxElemLimit`).

Complexity:
- **Time**: O(N), where N is the length of the link name string. Reading file handles is O(1) (fixed size).
- **Space**: O(N), where N is the length of the link name string, which is allocated internally by `file_name`.

Determinism:
- **Deterministic**: Given the same input byte stream, the function will always produce the same `link::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`**: This mechanism reads a fixed-size opaque byte array from the stream and validates its length against `NFS3_FHSIZE`. It is used twice in this module: once for the source file and once for the target directory.
 - **`file_name`**: This mechanism reads a length-prefixed string from the stream, validates it against protocol limits, and constructs a `vfs::file::Name`. It provides the actual name for the hard link being created.

- **From `nfs_mamont::vfs`**:
 - **`DirOpArgs`**: This mechanism acts as a standard container for directory operations. By using it, this module ensures that the output conforms to the interface expected by other VFS operations (like `CREATE` or `REMOVE`), which also use `DirOpArgs` to specify a target directory and entry name.

- **From `nfs_mamont::vfs::link`**:
 - **`Args`**: This mechanism defines the specific contract for the `LINK` procedure. It requires a source file handle and a `DirOpArgs`. This module is responsible for populating this specific structure.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a constructor for the `link::Args` entity defined in `nfs_mamont::vfs::link`.

Relations:
- **Transformation**: The `args` function transforms a byte stream (`Read`) into a `link::Args` struct.
- **Composition**: `link::Args` is composed of a `file::Handle` (source) and a `vfs::DirOpArgs` (target). The `vfs::DirOpArgs` is itself composed of a `file::Handle` (directory) and a `file::Name` (link name).

Global Invariants:
- **Field Order**: The parsing order (source file, target directory, link name) is strictly defined by the NFSv3 protocol specification and must match the order in which the client serializes the data.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. Specific variants likely returned include:
 - `Error::IO`: Underlying read error.
 - `Error::BadFileHandle`: Invalid file handle size.
 - `Error::MaxElemLimit`: Link name too long.
 - `Error::EnumDiscMismatch`: If internal parsing of types fails (though less likely for handles/strings).

Error Propagation Strategy:
- **Immediate Propagation**: The module uses the `?` operator on the results of `file::handle` and `file_name`. If any step fails, the error is returned immediately, and the construction of `link::Args` is aborted.

Recoverability:
- **Unrecoverable**: If parsing fails, the arguments cannot be constructed, and the RPC request cannot be processed. The caller (typically the RPC dispatcher) must handle the error, usually by sending an RPC rejection response to the client.

Panics:
- **Allowed**: No.
- **Conditions**: The code does not contain any `unwrap`, `expect`, or `panic!` calls. All potential failures are handled via the `Result` type.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **implement the deserialization logic for the NFSv3 `LINK` procedure**, translating raw network bytes into the structured arguments required by the Virtual File System (VFS) to create hard links. The system contains a layered architecture where the network layer receives raw bytes, the parser layer (this module) interprets those bytes according to the NFSv3 protocol, and the VFS layer executes the actual file system operation.

A typical usage scenario of the system involves a client sending an `LINK` request to create a new name for an existing file. The RPC dispatcher identifies the procedure number as `LINK` and invokes the `args` function in this module. The function reads the source file handle, the target directory handle, and the new name from the stream. It packages these into a `link::Args` struct. This struct is then passed to the VFS `Link` trait implementation, which performs the actual hard link creation on the storage backend.

Inside the system, the following things happen and they use this module:
- **Protocol Compliance**: The module enforces the specific wire format of the `LINK` arguments. By delegating the low-level byte reading to `file::handle` and `file::file_name`, it ensures that the data conforms to XDR standards (e.g., correct padding, length prefixes) and VFS constraints (e.g., valid filenames).
- **Interface Bridging**: The module bridges the generic `Read` stream with the specific `link::Args` domain model. It aggregates the parsed directory handle and name into a `DirOpArgs` struct, demonstrating how the parser adapts the raw data to fit the standardized input format expected by directory-modifying VFS operations.
- **Error Handling**: The module acts as the first line of defense against malformed requests. If the client sends a handle that is too short or a name that is too long, the parsing functions called by this module will fail, preventing invalid data from reaching the core VFS logic.

Without this module, the RPC dispatcher would lack a dedicated way to parse `LINK` requests, leading to either duplicated parsing code or a violation of the separation of concerns between protocol interpretation and file system logic. This module ensures that the specific semantics of the `LINK` operation (source file + target directory + new name) are correctly extracted from the network stream.