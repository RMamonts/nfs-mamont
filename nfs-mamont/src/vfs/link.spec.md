<!-- SPEC_HASH: 02d1461619c67d8c51048905d54a1d5e5eae937a80bc3eb877cd2f82b585d977 -->
# Module Specification

Module: nfs_mamont::vfs::link
Rust File: src/vfs/link.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import `WccData` and `Error`. `WccData` is required to report the state of the directory before and after the link operation for cache consistency. `Error` is used to report specific protocol-level failures (e.g., cross-device links).
- **`crate::vfs::file`**: Used to import `Attr` and `Handle`. `Handle` identifies the source file and the target directory. `Attr` is used to return the post-operation attributes of the source file.
- **`super::file`**: An alias for `crate::vfs::file`, used to access the file system primitives defined in the parent module scope.
- **`trait_variant::make`**: Used to generate a `Send` version of the `Link` trait. This is necessary because the trait defines an `async` method, and the resulting trait object must be safe to send across threads in the asynchronous runtime.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `LINK` procedure, which creates a hard link (a new directory entry) for an existing file.
- To enforce the protocol requirements regarding cross-device linking and illegal filenames via the contract defined in the trait documentation.
- To provide the necessary data structures to return Weak Cache Consistency (WCC) information and file attributes to the client, regardless of whether the operation succeeded or failed.

Inputs:
- **`args: Args`**: A structure containing:
 - `file`: A `file::Handle` identifying the existing file to be linked.
 - `link`: A `vfs::DirOpArgs` structure containing the handle of the directory where the link will be created and the name for the new link.

Outputs:
- **`Result<Success, Fail>`**:
 - **`Success`**: Contains `file_attr` (post-operation attributes of the source file) and `dir_wcc` (WCC data for the directory).
 - **`Fail`**: Contains `error` (the specific `vfs::Error`), `file_attr` (optional attributes of the source file), and `dir_wcc` (WCC data for the directory).

Steps:
1. **Validation**: The implementation of `Link` must verify that the source file (`args.file`) and the target directory (`args.link.dir`) reside on the same file system (i.e., their `fsid` attributes match).
2. **Name Validation**: The implementation must check if the new link name (`args.link.name`) is illegal (e.g., "." or "..") or if it creates an alias for the target directory.
3. **Execution**: If validations pass, the hard link is created in the storage backend.
4. **Data Collection**:
 - The attributes of the source file are retrieved (specifically the link count should be incremented).
 - The WCC data for the target directory is collected (pre-operation attributes if available, and post-operation attributes).
5. **Response**: The method returns `Success` with the collected data. If any step fails, it returns `Fail` containing the error and any available attributes/WCC data.

Edge Cases:
- **Cross-Device Link**: If the source file and target directory are on different file systems, the implementation must return `vfs::Error::XDev`.
- **Illegal Names**: If the link name is "." or "..", or an alias for the parent directory, the implementation must return `vfs::Error::InvalidArgument`.
- **Partial Failure**: The `Fail` struct includes `file_attr` and `dir_wcc`. The NFSv3 protocol allows returning these attributes even on failure to help the client synchronize its cache, though they may be `None` if the server could not retrieve them.

Complexity:
- **Time**: O(1) for the interface definition. The complexity of the actual operation depends on the backend implementation (typically O(1) to O(N) for directory updates).
- **Space**: O(1) for the data structures defined (`Args`, `Success`, `Fail`).

Determinism:
- **Deterministic**: The interface defines a strict contract for inputs and outputs. The behavior of the implementation is expected to be deterministic given the same file system state.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: Used in `Args` to uniquely identify the source file and the target directory. The `Handle` acts as the primary key for all VFS operations.
 - **`Attr`**: Used in `Success` and `Fail` to return the metadata of the source file. This is critical for the client to update its view of the file (e.g., noticing the incremented link count).
 - **`WccAttr`**: Implicitly used within `vfs::WccData` (referenced in `Success` and `Fail`) to represent the pre-operation state of the directory.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: Used in `Success` and `Fail` to provide Weak Cache Consistency data. This mechanism allows the client to validate cached directory contents without re-reading the entire directory.
 - **`DirOpArgs`**: Used in `Args` to standardize the arguments for directory operations (directory handle + entry name).
 - **`Error`**: Used in `Fail` to report specific error conditions like `XDev` (cross-device link) or `InvalidArgument` (illegal name), ensuring the client receives standard NFSv3 status codes.

---

## 4. Data Model

Entities:
- **`Args`**: Arguments for the link operation.
 - `file`: `file::Handle` (The existing file).
 - `link`: `vfs::DirOpArgs` (Target directory and new name).
- **`Success`**: Result of a successful link operation.
 - `file_attr`: `Option<file::Attr>` (Post-op attributes of the source file).
 - `dir_wcc`: `vfs::WccData` (Cache consistency data for the directory).
- **`Fail`**: Result of a failed link operation.
 - `error`: `vfs::Error` (The reason for failure).
 - `file_attr`: `Option<file::Attr>` (Attributes of the source file, if available).
 - `dir_wcc`: `vfs::WccData` (Cache consistency data for the directory).

Relations:
- **`Args` → `file::Handle`**: Composition.
- **`Args` → `vfs::DirOpArgs`**: Composition.
- **`Success` → `file::Attr`**: Optional association.
- **`Success` → `vfs::WccData`**: Composition.
- **`Fail` → `vfs::Error`**: Composition.
- **`Fail` → `file::Attr`**: Optional association.
- **`Fail` → `vfs::WccData`**: Composition.

Global Invariants:
- **Cross-Device Restriction**: The `Link` trait contract dictates that `args.file` and `args.link.dir` must be on the same file system. If not, `Error::XDev` is returned.
- **Name Restrictions**: The link name must not be "." or "..", nor an alias for the target directory.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type wrapped in the `Fail` struct. Relevant variants for this operation include:
 - `XDev`: Attempted to link across file systems.
 - `InvalidArgument`: The link name is illegal (e.g., ".", "..") or creates a directory alias.
 - `NoEntry`: The source file or target directory does not exist.
 - `IO`: A low-level I/O error occurred.
 - `ReadOnlyFs`: The file system is read-only.
 - `Access`: Permission denied.

Error Propagation Strategy:
- **Result Type**: The method returns `Result<Success, Fail>`. All error conditions are encapsulated in the `Fail` struct, which includes the error code and any available state data (attributes, WCC).

Recoverability:
- **Client-Side**: Recoverability depends on the specific error. `XDev` and `InvalidArgument` are permanent failures for the specific request arguments. `IO` errors might be transient and retryable by the client.

Panics:
- **Allowed**: No. The trait definition does not specify panics; implementations should return `Fail` with an appropriate `vfs::Error` instead.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`Link`**: An asynchronous trait with a `Send` variant. It defines the `link` method for creating hard links.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **implement the NFSv3 `LINK` procedure**, which allows clients to create hard links (additional names) for existing files within the file system. The system contains a complex architecture where file system operations are abstracted behind traits to allow for different storage backends and to strictly enforce protocol semantics. This module defines the specific contract for hard linking, ensuring that the operation adheres to NFSv3 constraints, such as preventing cross-device links and handling cache consistency data.

A typical usage scenario of the system involves a client sending an `LINK` request to the server. The RPC layer parses this request into `Args` (containing the source file handle and the target directory/name). The server then invokes the `link` method on an object implementing the `Link` trait (which is part of the larger `Vfs` trait aggregate). The implementation checks if the operation is valid (e.g., same file system), creates the link, and gathers the necessary metadata. The `Success` or `Fail` result is then serialized and sent back to the client, providing it with the updated file attributes and directory state to maintain cache coherence.

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: The `Link` trait documentation explicitly defines error conditions like `XDev` (cross-device) and `InvalidArgument` (illegal names). Implementers of this trait must return these specific errors, ensuring that the server's behavior complies with the NFSv3 RFC regardless of the underlying storage backend.
2.  **Cache Synchronization**: The module integrates with the `vfs::WccData` mechanism. By requiring `dir_wcc` in both `Success` and `Fail` results, the system ensures that the client receives the pre- and post-operation attributes of the directory where the link was created. This is critical for the client to validate its cached directory listing without performing a full `READDIR` operation.
3.  **Attribute Propagation**: The module requires `file_attr` in the result. This allows the client to see the updated attributes of the source file (specifically the increment in the hard link count) immediately after the operation, keeping the client's view of the file system consistent with the server's state.

Without this module, the VFS layer would lack a standardized way to perform hard links, leading to inconsistent implementations across different backends and potential protocol violations. This module centralizes the definition of the `LINK` operation, ensuring that all storage backends plugged into the server provide a uniform interface for this specific file system capability.