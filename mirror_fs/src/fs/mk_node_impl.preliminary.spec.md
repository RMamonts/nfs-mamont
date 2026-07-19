<!-- SPEC_HASH: dc9be12adb64307a13178c59cc5f8418c328cb4de81516d52b13a44012048924 -->
# Module Specification

Module: mirrorfs::fs::mk_node_impl
Rust File: mirror_fs/src/fs/mk_node_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs`**: Used to import the `Error` enum and `WccData` struct. `Error::NotSupported` is required to signal that the `MirrorFS` implementation does not support the creation of special nodes. `WccData` is required to construct the `Fail` response structure, even though the fields are populated with `None` in this implementation.
- **`nfs_mamont::vfs::mk_node`**: Used to import the `MkNode` trait, `Args`, `Success`, and `Fail` types. The module implements the `MkNode` trait for `MirrorFS` to satisfy the VFS interface contract, using the specific types to define the method signature and return values.
- **`super::MirrorFS`**: The parent struct for which the `MkNode` trait is being implemented. This allows `MirrorFS` to act as a valid VFS backend within the larger system.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a concrete implementation of the `MkNode` trait for the `MirrorFS` struct that explicitly signals the lack of support for creating special file system nodes (character devices, block devices, sockets, FIFOs). This ensures that the `MirrorFS` can satisfy the `Vfs` super-trait bounds (which require `MkNode`) without actually providing the functionality.

Inputs:
- `&self`: A reference to the `MirrorFS` instance.
- `_args: mk_node::Args`: The arguments describing the node to be created (type, attributes, location). This parameter is explicitly ignored (prefixed with `_`).

Outputs:
- `Result<mk_node::Success, mk_node::Fail>`: Always returns `Err(mk_node::Fail)`.

Steps:
1. The `mk_node` function is invoked on a `MirrorFS` instance.
2. The implementation ignores the provided arguments (`_args`).
3. It constructs a `mk_node::Fail` struct containing:
   - `error`: Set to `vfs::Error::NotSupported`.
   - `dir_wcc`: Set to `vfs::WccData { before: None, after: None }`.
4. The function returns this `Fail` struct wrapped in `Err`.

Edge Cases:
- **Ignored Arguments**: The implementation does not validate the input arguments (e.g., checking if the directory exists or if the name is valid) because it fails immediately with `NotSupported`.
- **Empty WCC Data**: The `dir_wcc` field is populated with `None` for both `before` and `after` attributes. This implies that the implementation does not perform any lookup or attribute retrieval for the parent directory before failing.

Complexity:
- Time: O(1).
- Space: O(1).

Determinism:
- Deterministic. The function always returns the same error and empty WCC data regardless of input.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::mk_node`**:
 - **`MkNode` Trait**: This module implements this trait to fulfill the interface requirement. The trait definition dictates that the function must be `async` and return a `Result<Success, Fail>`.
 - **`Fail` Struct**: This module uses this struct to wrap the error. The trait specification requires that `Fail` contains both an `error` and `dir_wcc`, which this module populates with `NotSupported` and empty `WccData` respectively.

- **From `nfs_mamont::vfs`**:
 - **`Error::NotSupported`**: This module uses this specific enum variant to indicate that the `MKNODE` operation is not available on this filesystem. The dependency specification notes that this error should be returned if the server does not support the operation at all.
 - **`WccData`**: This module uses this struct to satisfy the protocol requirement of returning directory state information. While the dependency spec emphasizes the importance of WCC for cache consistency, this implementation provides empty data, likely because the operation is rejected before any directory state is accessed.

---

## 4. Data Model

Entities:
- No new entities are defined in this module.

Relations:
- **Implementation**: `MirrorFS` implements `mk_node::MkNode`.

Global Invariants:
- **Constant Failure**: The `mk_node` operation on `MirrorFS` will always fail with `vfs::Error::NotSupported`.

## 5. Error Model

Error Types:
- **`vfs::Error::NotSupported`**: The only error returned by this module.

Error Propagation Strategy:
- **Direct Construction**: The error is constructed directly within the function and wrapped in the `mk_node::Fail` struct.

Recoverability:
- **Not Recoverable**: The error `NotSupported` indicates a permanent lack of capability for this specific filesystem instance. The client should not retry the operation expecting success.

Panics:
- **Allowed**: No.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::mk_node::MkNode`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **explicitly disable the creation of special file system nodes** (such as device files, sockets, or named pipes) within the `MirrorFS` implementation. The system requires all VFS backends to implement the `MkNode` trait to be compatible with the generic `Vfs` super-trait used by the NFS server. However, not all filesystem backends are capable of or designed to support these operations (e.g., a filesystem that mirrors a read-only source or a simple in-memory filesystem).

This system contains a `MirrorFS` struct that acts as a storage backend. To integrate this backend into the NFS server, it must satisfy the `Vfs` interface, which aggregates all NFSv3 procedure traits, including `MkNode`. This module provides the "glue" code that allows `MirrorFS` to compile and function as a `Vfs` backend without actually implementing the logic for creating special nodes.

A typical usage scenario of the system involves an NFS client sending a `MKNOD` request to create a character device. The request reaches the `MirrorFS` backend via the `mk_node` method defined in this module. Instead of attempting to modify the filesystem, the implementation immediately returns a `NotSupported` error. This informs the client that the operation is invalid for this specific export, preventing the client from wasting resources on retries or unsupported operations.

Inside the system, the following things happen and they use this module:
1. **Interface Compliance**: The Rust compiler requires `MirrorFS` to implement `MkNode` because it is used as a `Vfs` backend. This module satisfies that compiler requirement.
2. **Capability Signaling**: By returning `vfs::Error::NotSupported`, the module adheres to the NFSv3 specification's guidelines for handling unsupported procedures, ensuring the client receives a valid protocol error rather than a generic failure or a crash.
3. **Resource Conservation**: By failing fast and ignoring arguments, the module ensures that no resources (file handles, locks, or I/O operations) are consumed for an operation that the filesystem is designed to reject.

The critical aspect of this module is that it decouples the *interface requirement* (implementing the trait) from the *functional capability* (actually creating nodes). It allows `MirrorFS` to participate in the VFS ecosystem while honestly reporting its limitations.