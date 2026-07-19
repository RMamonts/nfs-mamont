<!-- SPEC_HASH: 02d1461619c67d8c51048905d54a1d5e5eae937a80bc3eb877cd2f82b585d977 -->
# Module Specification

Module: nfs_mamont::vfs::link
Rust File: src/vfs/link.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import `vfs::WccData`, `vfs::DirOpArgs`, and `vfs::Error`. `WccData` is essential for providing weak cache consistency information for the directory where the link is created. `DirOpArgs` defines the target location (directory handle and name). `vfs::Error` provides the specific error codes (such as `XDev` for cross-device links and `InvalidArgument` for illegal names) required by the NFSv3 protocol.
- **super::file**: Used to import `file::Handle` and `file::Attr`. `file::Handle` identifies the source file to be linked and the target directory. `file::Attr` is used to return the post-operation attributes of the source file (reflecting the updated link count).
- **trait_variant::make**: Used to generate a `Send`-compatible version of the `Link` trait. This allows the trait object to be passed between threads, which is necessary for an asynchronous server environment where tasks may be moved across threads.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for creating hard links within the Virtual File System (VFS) layer, strictly adhering to the NFSv3 protocol semantics.
- To ensure that implementations of this trait handle cross-device link restrictions and illegal filename constraints correctly.
- To provide a mechanism for returning detailed cache consistency data (`WccData`) and file attributes (`Attr`) even in the event of a failure, allowing clients to maintain cache coherence.

Inputs:
- `Args`: A struct containing:
  - `file`: A `file::Handle` representing the existing file system object to be linked.
  - `link`: A `vfs::DirOpArgs` struct representing the directory where the link will be created and the name of the new link.

Outputs:
- `Result<Success, Fail>`:
  - `Success`: Contains the post-operation attributes of the source file and `WccData` for the target directory.
  - `Fail`: Contains the specific `vfs::Error`, the post-operation attributes of the source file (if available), and `WccData` for the target directory.

Steps:
1. The `link` method is called with a reference to `Args`.
2. The implementation verifies that the source file (`Args::file`) and the target directory (`Args::link.dir`) reside on the same file system (checking `fsid`).
3. If they are on different file systems, the operation fails with `vfs::Error::XDev`.
4. The implementation checks if the new link name (`Args::link.name`) is legal (e.g., not "." or "..", and not an alias for the target directory).
5. If the name is illegal, the operation fails with `vfs::Error::InvalidArgument`.
6. If checks pass, the implementation creates the hard link in the underlying storage, incrementing the link count of the source file.
7. The implementation gathers the post-operation attributes of the source file and the `WccData` (before and after attributes) for the target directory.
8. The method returns `Result<Success, Fail>` populated with the gathered data.

Edge Cases:
- **Cross-device links**: Attempting to link a file to a directory on a different file system results in `Error::XDev`.
- **Illegal names**: Attempting to create a link named ".", "..", or an alias for the target directory results in `Error::InvalidArgument`.
- **Attribute availability**: The `file_attr` field in both `Success` and `Fail` is `Option<file::Attr>`, implying that attributes might not be returned if the file handle is stale or invalid before the operation can fully complete.

Complexity:
- Time: Unspecified by the interface, depends on the underlying file system implementation.
- Space: O(1) for the interface structures, though the underlying operation may allocate resources.

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs` (Assumption based on `vfs/mod.rs` facts)**:
  - `DirOpArgs`: Provides the structure to pass the target directory handle and the new entry name. It encapsulates the location where the hard link will appear.
  - `WccData`: Provides the structure to return weak cache consistency data. This includes `before` (pre-operation) and `after` (post-operation) attributes of the directory, allowing the client to validate its cache without re-reading the entire directory.
  - `Error`: Defines the enumeration of possible errors, specifically `XDev` (cross-device link) and `InvalidArgument` (illegal name), which are explicitly documented in the `Link` trait.

- **From `crate::vfs::file`**:
  - `Handle`: Acts as the opaque identifier for both the source file and the target directory. The implementation relies on the validity of these handles to perform the operation.
  - `Attr`: Represents the file attributes. In the context of linking, the `nlink` (number of hard links) field within `Attr` is expected to be incremented in the post-operation attributes returned to the client.

---

## 4. Data Model

Entities:
- **Args**: A structure holding the arguments for the link operation.
  - `file`: `file::Handle` - The handle of the existing file.
  - `link`: `vfs::DirOpArgs` - The target directory and the new name.
- **Success**: A structure representing a successful link operation result.
  - `file_attr`: `Option<file::Attr>` - Attributes of the source file after the link.
  - `dir_wcc`: `vfs::WccData` - Cache consistency data for the target directory.
- **Fail**: A structure representing a failed link operation result.
  - `error`: `vfs::Error` - The specific error that occurred.
  - `file_attr`: `Option<file::Attr>` - Attributes of the source file (if available).
  - `dir_wcc`: `vfs::WccData` - Cache consistency data for the target directory.
- **Link**: A trait defining the asynchronous interface for creating hard links.

Relations:
- **Composition**: `Args` aggregates `file::Handle` and `vfs::DirOpArgs`.
- **Composition**: `Success` and `Fail` aggregate `Option<file::Attr>` and `vfs::WccData`.
- **Association**: `Link` trait uses `Args` as input and produces `Result<Success, Fail>`.

Global Invariants:
- **Link Semantics**: The `file_attr` returned in `Success` must reflect the state of the file *after* the link is created (specifically, the link count should be incremented).
- **Error Handling**: Even if the operation fails (e.g., due to `XDev`), `Fail` should contain `dir_wcc` to allow the client to update its cache for the directory involved in the attempt.

## 5. Error Model

Error Types:
- `vfs::Error`

Error Propagation Strategy:
- The module uses the `Fail` struct to wrap the error. This struct does not simply propagate the error but bundles it with `file_attr` and `dir_wcc`. This pattern is specific to NFSv3 to allow clients to update their caches even when an operation fails.

Recoverability:
- Recoverable. The client receives the `Fail` struct, which contains the specific error code (e.g., `XDev`, `InvalidArgument`), allowing the client to decide whether to retry, abort, or notify the user.

Panics:
- Allowed: No
- Conditions: The interface definition does not specify any panics. Implementations are expected to return `Err(Fail)` instead of panicking.

---

## 6. Traits

List which external traits this module implements:
- **Send**: Implemented for `Link` via `#[trait_variant::make(Send)]`. This allows the trait object to be sent across threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the contract for the NFSv3 `LINK` procedure within the `nfs_mamont` server. The system contains a Virtual File System (VFS) abstraction layer that translates high-level NFS protocol requests into specific storage backend operations. A typical usage scenario of the system involves a client requesting to create a hard link to an existing file. The server receives this request, extracts the source file handle and the target directory/name, and invokes the `Link::link` method defined in this module.

Inside the system, the following things happen and they use this module: The `Vfs` trait (defined in `vfs::mod.rs`) aggregates the `Link` trait along with other NFS operation traits. When a `LINK` request is processed, the system relies on the `Link` implementation to enforce protocol-specific rules, such as preventing cross-device hard links (returning `Error::XDev`) and rejecting illegal names like "." or ".." (returning `Error::InvalidArgument`). Furthermore, the system uses the `Success` and `Fail` structs defined here to construct the NFS response packet. These structs ensure that the response includes not just the status of the operation, but also the `WccData` (Weak Cache Consistency data) for the directory. This is critical for the system because it allows NFS clients to maintain consistent caches of directory contents without performing expensive `LOOKUP` operations after every modification, thereby optimizing overall network performance and data integrity. Without this module, the VFS layer would lack a standardized way to handle hard linking semantics and cache consistency updates specific to the NFSv3 protocol.