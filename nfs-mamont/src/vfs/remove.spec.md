<!-- SPEC_HASH: 8f66b5273513df71959c2d8776d064196c05ca8dbef2128520a5a79c84e374e3 -->
# Module Specification

Module: nfs_mamont::vfs::remove
Rust File: src/vfs/remove.rs

---

## 1. Dependencies

From the source code and context, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the core types `vfs::Error`, `vfs::WccData`, and `vfs::DirOpArgs`. These types define the input arguments, the error reporting mechanism, and the cache consistency data structures required by the NFSv3 protocol.
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` attribute to transform the async `Remove` trait into a version that is safe to send across threads. This is necessary because the trait will likely be used as a trait object (`dyn Remove`) within an asynchronous, multi-threaded runtime (like Tokio), requiring the object to be `Send`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `REMOVE` procedure, which is responsible for deleting a file system object (typically a file) from a directory.
- To enforce the contract that implementations must return Weak Cache Consistency (WCC) data for the parent directory, regardless of whether the operation succeeds or fails. This allows clients to validate their cached directory contents.

Inputs:
- **`args: Args`**: A structure containing `object`, which is a `vfs::DirOpArgs`. This identifies the target directory (via a file handle) and the name of the entry to be removed.

Outputs:
- **`Result<Success, Fail>`**:
    - **`Ok(Success)`**: Indicates the entry was successfully removed. Contains `wcc_data` (attributes of the directory before and after the operation).
    - **`Err(Fail)`**: Indicates the operation failed. Contains `error` (a `vfs::Error` variant) and `dir_wcc` (attributes of the directory before the operation, and possibly after, depending on the error).

Steps:
1. The `remove` method is invoked on an implementation of the `Remove` trait with the provided `Args`.
2. The implementation attempts to unlink the specified entry from the directory.
3. **Success Path**: If successful, the implementation captures the directory attributes post-operation and returns `Success` with the full `WccData`.
4. **Failure Path**: If an error occurs (e.g., permission denied, entry not found), the implementation captures the error and the directory attributes (usually pre-operation) and returns `Fail`.

Edge Cases:
- **Non-empty Directories**: While the NFSv3 `REMOVE` procedure is strictly for files, if a client attempts to `REMOVE` a directory, the implementation should return an error (likely `vfs::Error::NotDir` or `vfs::Error::IsDir` or `vfs::Error::NotEmpty` depending on the specific filesystem semantics, though `RMDIR` is the correct procedure for directories).
- **Stale Handles**: If the directory handle in `Args` is invalid or stale, the operation must fail with `vfs::Error::StaleFile`.

Complexity:
- **Time**: O(1) for the interface definition. The complexity of the actual removal is delegated to the implementor of the trait.
- **Space**: O(1) for the data structures defined (`Args`, `Success`, `Fail`).

Determinism:
- **Deterministic**: The module defines a strict interface. The behavior is entirely determined by the implementation of the `Remove` trait.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
    - **`vfs::DirOpArgs`**: This structure is used as the input payload (`Args::object`). It encapsulates the directory handle and the filename, providing a standardized way to identify the target of the removal operation across all VFS modules.
    - **`vfs::WccData`**: This structure is used in both `Success` and `Fail` results. It is the carrier for Weak Cache Consistency information, which is a mandatory part of the NFSv3 protocol to ensure cache coherency between the server and clients.
    - **`vfs::Error`**: This enumeration is used in the `Fail` struct to report specific failure reasons (e.g., `NoEntry`, `Access`, `IO`). It provides a unified error language across the VFS layer.

---

## 4. Data Model

Entities:
- **`Args`**: Arguments for the remove operation.
    - `object`: `vfs::DirOpArgs` (Directory handle and entry name).
- **`Success`**: Result of a successful removal.
    - `wcc_data`: `vfs::WccData` (Directory attributes before and after).
- **`Fail`**: Result of a failed removal.
    - `error`: `vfs::Error` (The specific error that occurred).
    - `dir_wcc`: `vfs::WccData` (Directory attributes, typically pre-operation).
- **`Remove`**: An asynchronous trait defining the removal capability.

Relations:
- **Composition**: `Args` composes `vfs::DirOpArgs`.
- **Composition**: `Success` composes `vfs::WccData`.
- **Composition**: `Fail` composes `vfs::Error` and `vfs::WccData`.
- **Dependency**: The `Remove` trait is a dependency of the `nfs_mamont::vfs::Vfs` super-trait (as seen in the parent module specification).

Global Invariants:
- **WCC Requirement**: Both `Success` and `Fail` contain `WccData`. This invariant ensures that the client always receives directory attribute updates, allowing it to synchronize its cache even if the requested operation failed.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type embedded in the `Fail` struct. It covers standard NFSv3 errors (e.g., `NoEntry`, `Access`, `IO`) and implementation-specific errors.

Error Propagation Strategy:
- **Struct Wrapping**: Errors are not thrown directly but wrapped inside the `Fail` struct alongside `dir_wcc`. This allows the caller to access both the error reason and the directory state at the time of failure.

Recoverability:
- **Dependent on Error**: Recoverability is determined by the specific `vfs::Error` variant. For example, `JUKEBOX` implies the client should retry, while `StaleFile` implies the client must perform a new lookup. `Access` errors are generally not recoverable without changing permissions.

Panics:
- **Allowed**: No. The interface is designed for error reporting via `Result`, not panicking.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`Remove`**: An asynchronous trait with a `remove` method. It is marked `Send` via `trait_variant`, allowing it to be used as a trait object in multi-threaded contexts.

---

## 7. Overview

This module is used in order to **define the contract for deleting filesystem objects** within the NFSv3 server implementation. It specifically handles the `REMOVE` procedure, which corresponds to deleting a file (as opposed to `RMDIR` for directories).

The system contains a complex architecture where the VFS (Virtual File System) layer is split into granular traits for each NFS procedure. This module provides the `Remove` trait, which is aggregated into the main `Vfs` super-trait defined in the parent module. The RPC layer uses this trait to dispatch deletion requests received from the network.

A typical usage scenario of the system involves a client sending an NFSv3 `REMOVE` request. The RPC dispatcher decodes the request into `Args` (containing the directory handle and filename). It then calls the `remove` method on the object implementing the `Vfs` trait. The backend implementation performs the actual filesystem unlink operation. Regardless of success or failure, the backend returns `WccData` for the directory. This data is critical for the system because it allows the client to update its cache of the directory listing (e.g., noticing that the file is gone) without having to perform an expensive `READDIR` operation immediately.

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: The module enforces the NFSv3 requirement that directory modification operations must return cache consistency data. By structuring `Success` and `Fail` to mandatorily include `WccData`, the compiler ensures that no implementation can omit this information.
2.  **Abstraction**: It abstracts the complexity of file deletion. The higher-level server logic does not need to know how to unlink files on different underlying filesystems (e.g., local ext4 vs. a remote object store); it only interacts with the `Remove` trait.
3.  **Error Contextualization**: By pairing `vfs::Error` with `dir_wcc` in the `Fail` struct, the module ensures that even failed operations provide useful state information to the client, which is essential for robust distributed file system behavior.

Without this module, the VFS layer would lack a standardized way to request file deletions, leading to inconsistent implementations and potential protocol violations regarding cache coherency.