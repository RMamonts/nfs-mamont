<!-- SPEC_HASH: 847d2854d97e0c7d843ba80e68f70984e219aeff7d6afd382d1c272939da6317 -->
# Module Specification

Module: nfs_mamont::vfs::read_link
Rust File: src/vfs/read_link.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `vfs::Error` enum. This type is used in the `Fail` struct to indicate the specific reason for the operation's failure (e.g., `InvalidArgument` if the target is not a symbolic link).
- **`super::file`**: Used to import the `file::Handle`, `file::Attr`, `file::Path`, and `file::Type` types. `Handle` is the input identifier, `Attr` is used to return metadata, `Path` is the type-safe wrapper for the link target, and `Type` is referenced in documentation to define valid operation constraints.
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` macro to automatically generate a `Send` version of the `ReadLink` trait. This allows the trait to be used as a trait object in contexts where thread safety (sending across threads) is required, which is typical for async VFS worker pools.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `READLINK` procedure, which retrieves the path data stored within a symbolic link.
- To enforce the protocol requirement that the operation is only valid for objects of type `Symlink`.
- To support Weak Cache Consistency (WCC) by allowing the return of file attributes in both success and failure cases.

Inputs:
- **`Args`**: A structure containing a `file::Handle`, which serves as the identifier for the symbolic link object to be read.

Outputs:
- **`Result<Success, Fail>`**:
 - **`Success`**: Contains the `data` (the target path as `file::Path`) and optionally the `symlink_attr` (post-operation attributes).
 - **`Fail`**: Contains the `error` (a `vfs::Error`) and optionally the `symlink_attr` (post-operation attributes).

Steps:
1. **Invocation**: The `read_link` method is called with a reference to `self` and the `Args` struct.
2. **Type Validation**: The implementation checks if the object identified by `args.file` is of type `file::Type::Symlink`.
3. **Execution**:
 - If the type is valid, the implementation reads the content of the symbolic link (the target path).
 - It retrieves the current attributes of the symbolic link.
 - It returns `Ok(Success { data, symlink_attr })`.
4. **Error Handling**:
 - If the object is not a symlink, the implementation returns `Err(Fail { symlink_attr, error: vfs::Error::InvalidArgument })`.
 - For other errors (e.g., I/O error, stale handle), it returns `Err(Fail { symlink_attr, error: ... })`.

Edge Cases:
- **Non-Symlink Target**: If the provided `file::Handle` refers to a regular file or directory, the operation must fail with `vfs::Error::InvalidArgument`.
- **Attribute Availability**: The `symlink_attr` field in both `Success` and `Fail` is `Option<file::Attr>`. This allows the server to return attributes if they are available, or omit them if the handle was completely invalid or the state was unrecoverable.

Complexity:
- **Time**: O(1) for the interface definition. The actual complexity depends on the backend implementation (e.g., filesystem lookup).
- **Space**: O(1) stack space for the arguments and return structures. The `file::Path` contained in `Success` allocates heap memory proportional to the length of the link target.

Determinism:
- **Deterministic**: The interface defines a strict contract where the output is purely a function of the input handle and the current state of the filesystem.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
 - **Type Safety**: The module relies on `file::Path` to encapsulate the result of the read operation. This ensures that the returned path is validated (e.g., length limits) before being exposed to the upper layers of the server.
 - **Metadata Handling**: The module uses `file::Attr` to return the post-operation attributes. This allows the client to update its cache regarding the symbolic link's metadata (size, modification time) regardless of whether the primary operation (reading the target) succeeded or failed.

- **From `nfs_mamont::vfs` (mod.rs)**:
 - **Error Standardization**: The module uses `vfs::Error` to report failures. By using the centralized error enum, the `ReadLink` operation integrates seamlessly with the broader VFS error handling logic, allowing the RPC layer to map specific errors (like `InvalidArgument`) to correct NFS status codes.
 - **Trait Aggregation**: The `ReadLink` trait is included as a super-trait requirement for `Vfs<B>`. This means that any storage backend claiming to be a `Vfs` must explicitly support reading symbolic links, ensuring protocol compliance.

---

## 4. Data Model

Entities:
- **`Args`**: Arguments for the read link operation.
 - Fields: `file: file::Handle`.
- **`Success`**: Successful result of the operation.
 - Fields: `symlink_attr: Option<file::Attr>`, `data: file::Path`.
- **`Fail`**: Failed result of the operation.
 - Fields: `symlink_attr: Option<file::Attr>`, `error: vfs::Error`.
- **`ReadLink`**: The trait defining the asynchronous operation.

Relations:
- **Composition**: `Args` composes `file::Handle`.
- **Composition**: `Success` and `Fail` both compose `Option<file::Attr>`.
- **Association**: `Success` is associated with `file::Path` (the link content).

Global Invariants:
- **Type Constraint**: The `read_link` operation is semantically valid only if the `file::Handle` refers to an object of type `file::Type::Symlink`. Implementations must enforce this.
- **Attribute Consistency**: If `symlink_attr` is present in `Success` or `Fail`, it represents the state of the symlink *after* the operation attempt (post-operation attributes).

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type wrapped in the `Fail` struct. Specific variants of interest include `InvalidArgument` (for type mismatches) and `IO` (for read failures).

Error Propagation Strategy:
- **Result Wrapper**: The method returns a `Result<Success, Fail>`. The `Fail` struct explicitly carries the `vfs::Error` and optional attributes, allowing the caller to distinguish between different failure modes while still accessing cache data.

Recoverability:
- **Dependent on Error**: 
 - `InvalidArgument`: Not recoverable by the server logic (client error).
 - `IO`: Potentially recoverable if transient (client may retry).

Panics:
- **Allowed**: No. The trait definition implies a fallible operation returning a `Result`, not a panic.

---

## 6. Traits

List which external traits this module implements:
- **`Send`**: Implemented for `ReadLink` via the `trait_variant` macro. This allows the trait object to be sent between threads.

List which traits this module defines:
- **`ReadLink`**: Defines the asynchronous interface for reading symbolic links.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **encapsulate the logic and protocol requirements for resolving symbolic links** within the `nfs_mamont` NFSv3 server. The system contains a complex architecture where file system operations are abstracted into individual traits to allow for modular implementation and testing. This module specifically addresses the `READLINK` NFSv3 procedure, ensuring that the server can correctly interpret the contents of a symlink file and return it to the client in a standardized format.

A typical usage scenario of the system involves a client navigating a directory structure that contains symbolic links. When the client issues a `READLINK` request for a specific file handle, the RPC layer dispatches this request to the VFS backend. The backend, implementing the `ReadLink` trait, takes the `file::Handle`, verifies it is indeed a symlink, and reads the target path. The critical aspect here is that the result is wrapped in `file::Path`, ensuring the returned string is a valid filesystem path, and accompanied by `file::Attr` to support cache consistency.

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: The `vfs` module uses this trait to ensure that any backend plugged into the server supports symlink resolution. The documentation within the trait explicitly mandates the use of `vfs::Error::InvalidArgument` for non-symlink objects, enforcing strict NFSv3 compliance.
2.  **Cache Management**: By including `symlink_attr` in both `Success` and `Fail` structs, this module enables the Weak Cache Consistency (WCC) mechanism. The upper layers of the server use these attributes to update the client's cache regarding the symlink's metadata (like modification time), even if the actual read operation failed (e.g., due to a transient I/O error).
3.  **Type Safety**: The module relies on `file::Path` to return the link data. This prevents the backend from returning invalid or malformed paths to the client, shifting the validation burden to the type system rather than manual checks in the RPC serialization layer.

Without this module, the VFS interface would lack a dedicated definition for reading symlinks, likely forcing the implementation to use generic read operations or ad-hoc structures. This would obscure the intent of the operation, make it harder to enforce protocol-specific error codes (like `InvalidArgument`), and complicate the handling of symlink-specific metadata. This module provides a clear, type-safe contract for symlink resolution.