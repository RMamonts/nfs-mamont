<!-- SPEC_HASH: dc9be12adb64307a13178c59cc5f8418c328cb4de81516d52b13a44012048924 -->
# Module Specification

Module: mirrorfs::fs::mk_node_impl
Rust File: mirror_fs/src/fs/mk_node_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs`**: Used to import the `Error` enum and `WccData` struct. `Error::NotSupported` is required to signal that the `MirrorFS` implementation does not support the creation of special files. `WccData` is required to construct the `Fail` struct, which must return directory cache consistency data even when the operation fails.
- **`nfs_mamont::vfs::mk_node`**: Used to import the `MkNode` trait, `Args`, `Success`, and `Fail` types. This module provides the implementation of the `MkNode` trait for the `MirrorFS` struct, allowing `MirrorFS` to be used as a VFS backend.
- **`super::MirrorFS`**: The type for which the `MkNode` trait is being implemented. *Assumption: `MirrorFS` is a filesystem backend implementation that acts as a storage provider within the `nfs_mamont` server.*

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To explicitly disable the `MKNODE` operation for the `MirrorFS` backend. By implementing the `MkNode` trait, the module ensures that `MirrorFS` satisfies the requirements of the `Vfs` super-trait (which requires `MkNode`), while the implementation logic strictly signals that this specific functionality is not available.

Inputs:
- `&self`: A reference to the `MirrorFS` instance.
- `_args: mk_node::Args`: The arguments for the operation (containing the target directory, name, and type of special file to create). This argument is explicitly ignored.

Outputs:
- `Result<mk_node::Success, mk_node::Fail>`: Always returns `Err(mk_node::Fail)`.

Steps:
1. The `mk_node` function is invoked on a `MirrorFS` instance via the `MkNode` trait.
2. The function constructs a `mk_node::Fail` struct.
3. The `error` field of the `Fail` struct is set to `vfs::Error::NotSupported`.
4. The `dir_wcc` field of the `Fail` struct is set to `vfs::WccData { before: None, after: None }`, indicating that no pre- or post-operation directory attributes are available or were retrieved.
5. The `Fail` struct is wrapped in `Err` and returned.

Edge Cases:
- **Ignored Arguments**: The implementation does not validate the input arguments (e.g., checking if the directory exists or if the name is valid). It fails immediately with `NotSupported` regardless of the input content.

Complexity:
- Time: O(1).
- Space: O(1).

Determinism:
- Deterministic. The function always returns the same error structure for any input.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::mk_node`**:
 - **`MkNode` Trait**: This module implements this trait. The trait defines the `mk_node` method which is the entry point for creating special files. The implementation here acts as a "stub" that fulfills the interface contract while denying the capability.
 - **`Fail` Struct**: This module constructs this struct to return the error to the caller. It populates the `error` and `dir_wcc` fields as required by the trait definition.

- **From `nfs_mamont::vfs`**:
 - **`Error::NotSupported`**: This specific variant is used to indicate that the `MirrorFS` backend does not implement the `MKNODE` procedure. This is distinct from `BadType` (which would imply the procedure exists but the specific file type is invalid) or `IO` errors.
 - **`WccData`**: This struct is used to satisfy the NFSv3 requirement of returning Weak Cache Consistency data. In this failure case, it is populated with `None` values, likely because the implementation does not perform any I/O or attribute lookup on the parent directory.

---

## 4. Data Model

Entities:
- This module defines no new entities. It implements a trait for an existing entity (`MirrorFS`).

Relations:
- **Implementation**: `MirrorFS` implements `mk_node::MkNode`.

Global Invariants:
- **Capability Denial**: `MirrorFS` will never successfully create a special file via the `mk_node` interface. It will always return `vfs::Error::NotSupported`.

## 5. Error Model

Error Types:
- **`vfs::Error::NotSupported`**: The only error returned by this module.

Error Propagation Strategy:
- **Direct Return**: The error is constructed and returned immediately within the `Err` variant of the `Result`.

Recoverability:
- **Not Recoverable**: The error `NotSupported` indicates a permanent lack of capability in the backend. The client should not retry the operation expecting it to succeed later.

Panics:
- **Allowed**: No.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::mk_node::MkNode`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **integrate the `MirrorFS` backend into the NFSv3 VFS layer by providing a concrete implementation of the `MkNode` trait**. The system requires all VFS backends to implement the `MkNode` trait (as it is part of the `Vfs` super-trait) to be compatible with the RPC dispatcher. However, the `MirrorFS` backend is designed (or limited) such that it does not support the creation of special file system nodes (character devices, block devices, sockets, or FIFOs). This module is necessary because it bridges the gap between the generic requirement (implementing the trait) and the specific limitation (lack of support), allowing the server to compile and run correctly while gracefully declining unsupported operations.

A typical usage scenario of the system involves an NFS client attempting to create a device node or a named pipe on a share exported via `MirrorFS`. The RPC layer receives the `MKNOD` request and dispatches it to the `MirrorFS` instance. Instead of attempting to create the file, this implementation intercepts the call and returns a `NotSupported` error. This prevents the server from crashing or panicking due to an unimplemented method and correctly informs the client that the operation is invalid for this specific filesystem.

Inside the system, the following things happen and they use this module:
1. **Trait Satisfaction**: The Rust compiler requires `MirrorFS` to implement `MkNode` to satisfy the `Vfs` trait bound used in the server context. This module provides that implementation.
2. **Capability Signaling**: By returning `vfs::Error::NotSupported`, the module adheres to the NFSv3 specification, which dictates that a server must return a specific error code if it does not support a procedure, rather than failing silently or crashing.
3. **Resource Conservation**: Since the arguments are ignored and no I/O is attempted, the module ensures that no resources (file handles, locks) are consumed for an operation that is known to be impossible.

*Assumption*: `MirrorFS` is a filesystem backend that likely handles regular files and directories but is incapable of handling special files, possibly due to the limitations of the underlying storage or the specific use-case of the mirror (e.g., mirroring to a destination that does not support devices).