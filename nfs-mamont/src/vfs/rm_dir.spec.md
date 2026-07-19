<!-- SPEC_HASH: a571d25cc4dcd8906d5068ee8d320b1755db21d21e4f37a8a6a56083603050db -->
# Module Specification

Module: nfs_mamont::vfs::rm_dir
Rust File: src/vfs/rm_dir.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `WccData`, `Error`, and `DirOpArgs` types. These types are essential for defining the input arguments, the success/failure payloads, and the error handling semantics specific to the NFSv3 protocol's Weak Cache Consistency model.
- **`trait_variant`**: Used to apply the `make(Send)` macro to the `RmDir` trait. This transforms the async trait into an object-safe trait that is also `Send`, allowing the trait to be used as a trait object in multi-threaded asynchronous contexts (e.g., across tasks in a Tokio runtime).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `RMDIR` procedure within the Virtual File System (VFS) abstraction layer.
- To encapsulate the specific arguments, success conditions, and failure modes associated with removing a subdirectory, including the handling of special directory entries ("." and "..") and the provision of Weak Cache Consistency (WCC) data.

Inputs:
- **`args: Args`**: A structure containing `object`, which is a `vfs::DirOpArgs`. This identifies the parent directory (via a file handle) and the name of the subdirectory to be removed.

Outputs:
- **`Result<Success, Fail>`**:
  - **`Ok(Success)`**: Indicates the directory was successfully removed. Contains `wcc_data` (post-operation attributes).
  - **`Err(Fail)`**: Indicates the operation failed. Contains `error` (the specific `vfs::Error`) and `dir_wcc` (pre-operation attributes of the directory).

Steps:
1. The `rm_dir` method is invoked on an implementation of the `RmDir` trait with the provided `Args`.
2. The implementation attempts to delete the directory entry specified in `args.object`.
3. **Special Case Handling**:
   - If the filename is `.`, the implementation must return `Err(Fail)` with `vfs::Error::InvalidArgument`.
   - If the filename is `..`, the implementation must return `Err(Fail)` with `vfs::Error::Exist`.
4. **Success Path**: If the removal is successful, the implementation returns `Ok(Success)`. The `Success` struct must contain `wcc_data` representing the state of the parent directory *after* the modification.
5. **Failure Path**: If the removal fails for any other reason (e.g., permission denied, not empty, I/O error), the implementation returns `Err(Fail)`. The `Fail` struct must contain the specific `error` and `dir_wcc` representing the state of the parent directory *before* the failed attempt (to allow cache validation).

Edge Cases:
- **Removing "."**: Explicitly defined to return `vfs::Error::InvalidArgument`.
- **Removing ".."**: Explicitly defined to return `vfs::Error::Exist`.
- **Non-empty Directory**: While not explicitly handled in the docstring of this module, standard NFSv3 behavior (implied by the dependency on `vfs::Error`) suggests returning `vfs::Error::NotEmpty` if the directory contains entries.

Complexity:
- **Time**: O(1) for the interface definition. The actual complexity depends on the backend implementation (filesystem lookup and deletion).
- **Space**: O(1) for the data structures defined (`Args`, `Success`, `Fail`).

Determinism:
- **Deterministic (Interface)**: The contract defined by the trait is deterministic regarding the mapping of inputs (".", "..") to specific errors.
- **Non-deterministic (Execution)**: The success or failure of the operation depends on the runtime state of the filesystem backend.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
  - **`WccData`**: This module relies on the `WccData` structure to implement the NFSv3 Weak Cache Consistency mechanism. In the `Success` case, it holds post-operation attributes; in the `Fail` case, it holds pre-operation attributes. This allows clients to validate their cached data without re-reading the entire directory.
  - **`DirOpArgs`**: This module uses `DirOpArgs` as the input payload. This standardizes how directory operations (identifying a parent directory and a target name) are represented across the VFS layer.
  - **`Error` Enum**: This module uses the `vfs::Error` enum to report specific protocol-level failures. The explicit mapping of "." to `InvalidArgument` and ".." to `Exist` demonstrates how this module utilizes the standardized error codes provided by the parent VFS module.

---

## 4. Data Model

Entities:
- **`Args`**: A wrapper structure holding the necessary information to perform the removal.
  - `object: vfs::DirOpArgs`: Identifies the directory and the entry name.
- **`Success`**: Represents a successful removal operation.
  - `wcc_data: vfs::WccData`: Cache consistency data for the directory after the removal.
- **`Fail`**: Represents a failed removal operation.
  - `error: vfs::Error`: The specific error that occurred.
  - `dir_wcc: vfs::WccData`: Cache consistency data for the directory before the failed operation.
- **`RmDir`**: An asynchronous trait defining the behavior of removing a directory.

Relations:
- **`Args` → `vfs::DirOpArgs` (Composition)**: `Args` owns a `DirOpArgs` instance.
- **`Success` → `vfs::WccData` (Composition)**: `Success` owns a `WccData` instance.
- **`Fail` → `vfs::Error` (Composition)**: `Fail` owns an `Error` instance.
- **`Fail` → `vfs::WccData` (Composition)**: `Fail` owns a `WccData` instance.

Global Invariants:
- The `RmDir` trait is `Send`, meaning any implementation must be safe to transfer across thread boundaries.
- The `wcc_data` in `Success` must reflect the post-operation state, while `dir_wcc` in `Fail` must reflect the pre-operation state (or the state that allows the client to detect the change).

## 5. Error Model

Error Types:
- **`vfs::Error`**: The enumeration of all possible NFSv3 and server-specific errors (e.g., `InvalidArgument`, `Exist`, `NotEmpty`, `IO`).

Error Propagation Strategy:
- **Explicit Result Type**: The trait method returns `Result<Success, Fail>`. Errors are not thrown as exceptions or panics but are wrapped in the `Fail` struct, which includes both the error code and the WCC data.

Recoverability:
- **Client-Side**: Recoverability is determined by the specific `vfs::Error` variant returned. For example, `JUKEBOX` implies the client should retry, while `Permission` implies the request should not be retried without changing credentials.

Panics:
- **Allowed**: No. The interface is designed to handle all error conditions via the `Result` type.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`RmDir`**: An asynchronous trait (made `Send`) that defines the contract for removing a subdirectory.

---

## 7. Overview

This module is used in order to **define the specific interface and contract for the NFSv3 RMDIR procedure** within the broader Virtual File System (VFS) architecture. The system contains a complex, modular VFS layer where individual file operations (like `Read`, `Write`, `Lookup`, `RmDir`) are defined in separate sub-modules to maintain separation of concerns and adhere to the Single Responsibility Principle.

A typical usage scenario of the system involves the NFS server receiving an RPC request for the `RMDIR` procedure. The RPC dispatcher, which operates on the generic `Vfs` trait, invokes the `rm_dir` method. The concrete implementation of the `RmDir` trait (provided by the filesystem backend) executes the logic to delete the directory. It returns a `Result<Success, Fail>`, which the server then serializes into the NFSv3 wire format, including the necessary `WccData` to ensure the client's cache remains consistent.

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: This module enforces specific NFSv3 semantics, such as the requirement that attempting to remove "." results in `InvalidArgument` and ".." results in `Exist`. This ensures that all backend implementations conform to the standard protocol behavior.
2.  **Cache Consistency Management**: By requiring `WccData` in both success and failure cases, this module ensures that the backend provides the necessary metadata for the client to synchronize its cache. This is critical for performance and correctness in distributed file systems.
3.  **Trait Aggregation**: The `vfs` parent module aggregates the `RmDir` trait into its main `Vfs` super-trait. This allows the server to treat the filesystem as a single object that supports all operations, while the implementation details of `RmDir` remain encapsulated here.

Without this module, the definition of the `RmDir` operation would likely be mixed with other operations or defined in a less structured way, potentially leading to inconsistent error handling or missing cache consistency data. This module provides a clear, type-safe contract that backend implementers must fulfill.