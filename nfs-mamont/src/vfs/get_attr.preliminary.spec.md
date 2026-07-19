<!-- SPEC_HASH: 6c9d15284c44f32dad6d604849d60b9e23848aab41bd07c2266c48917748f626 -->
# Module Specification

Module: nfs_mamont::vfs::get_attr
Rust File: src/vfs/get_attr.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import the `vfs::Error` enum. This type is wrapped in the `Fail` struct to represent the specific failure modes of the VFS operation (e.g., `StaleFile`, `IO`) that correspond to NFSv3 status codes.
- **super::file** (crate::vfs::file): Used to import `file::Handle` and `file::Attr`. `Handle` serves as the input key to identify the file system object, while `Attr` serves as the output payload containing the object's metadata (type, size, timestamps, etc.).
- **trait_variant**: Used via the `#[trait_variant::make(Send)]` attribute macro. This transforms the `GetAttr` trait into an object-safe version that implements `Send`, allowing the trait to be used as a trait object in asynchronous contexts across threads.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface contract for the NFSv3 `GETATTR` procedure.
- To abstract the retrieval of file system metadata, decoupling the protocol handler from the specific storage backend implementation.
- To enforce type safety by wrapping inputs and outputs in strongly-typed structs (`Args`, `Success`, `Fail`) rather than passing raw tuples or primitives.

Inputs:
- `args: Args`: A structure containing a `file::Handle`, which is the opaque identifier for the file system object.

Outputs:
- `Result<Success, Fail>`:
  - `Ok(Success)`: Contains a `file::Attr` struct with the requested metadata.
  - `Err(Fail)`: Contains a `vfs::Error` indicating why the operation failed.

Steps:
1. The consumer (e.g., an NFS request handler) constructs an `Args` struct containing the `file::Handle` received from the network.
2. The consumer invokes the `get_attr` method on an implementation of the `GetAttr` trait.
3. The implementation resolves the `file::Handle` to a concrete file system object.
4. If the object exists and is accessible, the implementation retrieves its metadata and wraps it in the `Success` struct.
5. If the object does not exist, the handle is invalid, or an I/O error occurs, the implementation wraps the corresponding `vfs::Error` in the `Fail` struct.

Edge Cases:
- **Stale Handles**: The `file::Handle` may refer to a deleted object or an object that has been re-used. The interface allows returning `vfs::Error::StaleFile` to handle this.
- **Permission Issues**: If the caller lacks permission to read the attributes, the implementation can return `vfs::Error::Access` or `vfs::Error::Permission`.

Complexity:
- Time: Unspecified by this module (depends on the trait implementation).
- Space: O(1) for the interface definition (excluding the size of the returned `Attr` struct).

Determinism:
- Deterministic (The interface definition itself is deterministic; the determinism of the result depends on the implementation).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs::file`**:
  - **Handle**: Acts as the primary key for the operation. The module relies on the invariant that a `Handle` uniquely identifies a file system object within the context of the VFS implementation.
  - **Attr**: Acts as the data transfer object. The module relies on `Attr` to encapsulate all metadata fields required by the NFSv3 protocol (mode, uid, gid, size, timestamps, etc.) in a single serializable structure.
- **From `crate::vfs`**:
  - **Error**: Provides the enumeration of failure states. The module relies on the specific variants of `Error` (e.g., `NoEntry`, `IO`, `StaleFile`) to accurately report the nature of failures to the client.

---

## 4. Data Model

Entities:
- **Args**: A parameter struct holding a `file::Handle`. It represents the request context.
- **Success**: A result struct holding a `file::Attr`. It represents the successful retrieval of metadata.
- **Fail**: A result struct holding a `vfs::Error`. It represents a failed retrieval attempt.
- **GetAttr**: A trait defining the asynchronous operation signature. It is marked `Send` to allow concurrency.

Relations:
- **Usage**: `GetAttr::get_attr` takes `Args` as input and returns `Result<Success, Fail>`.
- **Composition**: `Args` composes `file::Handle`. `Success` composes `file::Attr`. `Fail` composes `vfs::Error`.

Global Invariants:
- **Result Invariants**: The `Result` type ensures that a call to `get_attr` yields either a valid `Success` containing a fully populated `Attr` or a `Fail` containing a valid `Error`, but never both or neither.

## 5. Error Model

Error Types:
- `vfs::Error` (wrapped in `Fail`)

Error Propagation Strategy:
- The interface uses Rust's `Result` type for explicit error handling. Errors are not thrown as exceptions but returned as the `Err` variant of the `Result`.

Recoverability:
- Recoverable. The caller is expected to inspect the `Result` and handle the `Fail` case, typically by mapping the `vfs::Error` to an appropriate NFSv3 status code to be sent back to the client.

Panics:
- Allowed: No
- Conditions: The trait definition itself does not panic. However, implementations of the trait might panic if invariants are violated (e.g., unsafe code), though this is not part of the interface specification.

---

## 6. Traits

List which external traits this module implements:
- **None**: This module defines the `GetAttr` trait but does not implement external traits on its own types (beyond standard derives implied by the structs).

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the standard interface for retrieving file metadata within the `nfs_mamont` NFSv3 server. The system contains a Virtual File System (VFS) layer that abstracts various storage backends (e.g., local disk, in-memory) from the NFS protocol logic. A typical usage scenario of the system involves an NFS client sending a `GETATTR` request containing a file handle. The server decodes this handle into a `file::Handle` and invokes the `get_attr` method defined in this module. The VFS implementation then translates this handle into backend-specific operations (e.g., `stat` syscall), retrieves the metadata, and constructs a `file::Attr` struct.

Inside the system, the following things happen and they use this module: The `Vfs` trait (defined in `vfs::mod.rs`) aggregates `GetAttr` along with other operation traits (like `Lookup`, `Read`, etc.) to form a complete file system interface. The `GetAttr` trait specifically ensures that any storage backend plugged into the server can provide the standardized set of attributes defined in `file::Attr` (type, permissions, size, timestamps) and can report errors using the standardized `vfs::Error` enum. This is crucial because the NFSv3 protocol requires specific attributes to be present in the response; without this module enforcing the return type `Success { object: file::Attr }`, different backends might return incompatible metadata formats, breaking protocol compliance. Furthermore, the use of `trait_variant` ensures that this operation can be performed asynchronously and sent across threads, which is necessary for the high-performance, concurrent architecture of the NFS server.