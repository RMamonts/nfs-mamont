<!-- SPEC_HASH: 847d2854d97e0c7d843ba80e68f70984e219aeff7d6afd382d1c272939da6317 -->
# Module Specification

Module: nfs_mamont::vfs::read_link
Rust File: src/vfs/read_link.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import the `vfs::Error` enum. This enum is utilized in the `Fail` struct to report specific protocol-level errors (e.g., `InvalidArgument`) when the `read_link` operation cannot be completed successfully.
- **crate::vfs::file**: Used to import core data types required for the operation's interface. Specifically, `file::Handle` is used to identify the target object, `file::Attr` is used to return metadata about the symbolic link, `file::Path` is used to represent the content (target) of the symbolic link, and `file::Type` is referenced in documentation to define the valid scope of the operation.
- **super::file**: This is a re-export or alias for `crate::vfs::file`, used to access the types listed above with a shorter path within the module.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the contract for the NFSv3 `READLINK` procedure within the VFS layer.
- To enforce type safety and protocol compliance by requiring that the operation is only performed on symbolic links.
- To provide a structure for returning both the result data (the link target) and the post-operation attributes, which is necessary for cache consistency in NFSv3.

Inputs:
- `Args`: A structure containing a `file::Handle` representing the file system object to be read.

Outputs:
- `Result<Success, Fail>`:
  - `Success`: Contains the `file::Path` (the data the symlink points to) and optional `file::Attr` (post-operation attributes).
  - `Fail`: Contains the `vfs::Error` explaining the failure and optional `file::Attr` (attributes if available despite the failure).

Steps:
1. **Trait Invocation**: A consumer of the `ReadLink` trait calls the `read_link` method with a `Args` struct containing the handle of the file system object.
2. **Type Validation (Contractual)**: The implementation of `read_link` must verify that the object identified by the handle is of type `file::Type::Symlink`.
3. **Execution**:
   - If the object is a symlink, the implementation reads the target path from the underlying storage.
   - It retrieves the current attributes of the symlink.
4. **Result Construction**:
   - On success, a `Success` struct is populated with the target path (`data`) and the attributes (`symlink_attr`).
   - If the object is not a symlink, a `Fail` struct is returned with `vfs::Error::InvalidArgument`.
   - For other errors (e.g., I/O errors, stale handles), a `Fail` struct is returned with the appropriate `vfs::Error`.

Edge Cases:
- **Non-Symlink Handle**: If the provided `file::Handle` refers to a regular file, directory, or any other non-symlink type, the operation must return `Err(Fail)` where `error` is `vfs::Error::InvalidArgument`.
- **Attribute Availability**: The `symlink_attr` field in both `Success` and `Fail` is `Option<file::Attr>`. This allows the implementation to return attributes if they are available, or omit them if the failure occurred before attributes could be retrieved (though NFSv3 usually encourages returning attributes in `Fail` for cache consistency).

Complexity:
- Time: Unspecified by this module (depends on the async implementation of the trait).
- Space: O(N) where N is the length of the symbolic link's target path (stored in `file::Path`).

Determinism:
- Deterministic (assuming the underlying file system state is stable).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs::file`**:
  - **Handle**: Provides the fixed-size byte array identifier used to locate the file system object without exposing internal paths.
  - **Path**: Provides a validated wrapper for the string data representing the target of the symbolic link. It ensures the returned path conforms to VFS length constraints.
  - **Attr**: Provides the structure for file metadata (type, mode, size, timestamps, etc.) required for the `symlink_attr` field.
  - **Type**: Provides the enumeration used to distinguish between different file system objects, specifically `Type::Symlink`, which is the target type for this operation.
- **From `crate::vfs`**:
  - **Error**: Provides the enumeration of error codes (e.g., `InvalidArgument`, `StaleFile`, `IO`) that map to NFSv3 status codes, allowing the `read_link` operation to signal specific failure modes to the client.

---

## 4. Data Model

Entities:
- **Args**: A message struct containing a single field `file` of type `file::Handle`. It represents the input arguments for the `read_link` operation.
- **Success**: A message struct representing a successful operation. It contains:
  - `symlink_attr`: `Option<file::Attr>` representing the attributes of the symlink after the operation.
  - `data`: `file::Path` representing the content of the symbolic link (the path it points to).
- **Fail**: A message struct representing a failed operation. It contains:
  - `symlink_attr`: `Option<file::Attr>` representing the attributes of the symlink (if available).
  - `error`: `vfs::Error` representing the specific error that occurred.

Relations:
- **Composition**: `Args` owns a `file::Handle`.
- **Composition**: `Success` owns a `file::Path` and optionally a `file::Attr`.
- **Composition**: `Fail` owns a `vfs::Error` and optionally a `file::Attr`.

Global Invariants:
- **Type Constraint**: The `read_link` operation is semantically valid only if the `Args.file` refers to an object of type `file::Type::Symlink`. If this invariant is violated by the input, the output must be a `Fail` with `vfs::Error::InvalidArgument`.
- **Path Validity**: The `data` field in `Success` must adhere to the invariants of `file::Path` (non-empty, length <= `MAX_PATH_LEN`).

## 5. Error Model

Error Types:
- `vfs::Error` (wrapped in the `Fail` struct).

Error Propagation Strategy:
- The trait method `read_link` returns a `Result<Success, Fail>`. Errors are not thrown as exceptions or panics but are explicitly wrapped in the `Fail` struct, which includes the error code and optional attributes.

Recoverability:
- Recoverable. The caller receives the `Fail` struct and can inspect the `error` field to determine the next action (e.g., propagating the error to the NFS client).

Panics:
- Allowed: No
- Conditions: The interface definition does not specify any panic conditions. Implementations should handle errors by returning `Fail`.

---

## 6. Traits

List which external traits this module implements:
- **ReadLink**: The main trait defined in this module. It is marked with `#[trait_variant::make(Send)]`, which implies that the trait is object-safe and can be used as a `dyn ReadLink + Send` trait object.
- **Send**: Automatically implemented for `ReadLink` via the `trait_variant` macro, allowing the trait to be passed between threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the specific interface for resolving symbolic links within the NFSv3 server implementation. The system contains a Virtual File System (VFS) layer that abstracts the complexities of various storage backends (local disk, network storage, etc.) behind a unified set of traits. A typical usage scenario of the system involves an NFS client sending a `READLINK` request for a specific file handle. The server decodes this request into the `Args` struct defined in this module and invokes the `read_link` method on the VFS implementation.

Inside the system, the following things happen and they use this module: The VFS implementation uses the `file::Handle` provided in `Args` to locate the inode or metadata object corresponding to the symbolic link. It then checks the `file::Type` (via `file::Attr`) to ensure the object is indeed a symlink. If it is, the system reads the link target, validates it as a `file::Path`, and returns it inside the `Success` struct. The inclusion of `symlink_attr` in both `Success` and `Fail` results is crucial for the NFS protocol's Weak Cache Consistency (WCC) mechanism, allowing clients to update their caches even if the operation fails or to verify that the object hasn't changed. Without this module, the VFS would lack a standardized way to request link resolution, making it impossible to support the NFSv3 `READLINK` procedure and consequently breaking interoperability with clients expecting to traverse symbolic links.