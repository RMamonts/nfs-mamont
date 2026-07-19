<!-- SPEC_HASH: f6ad4fd317d94d28f413c5fae549b70b042421fd4bb5a5ba8912e4e2bd30ec18 -->
# Module Specification

Module: nfs_mamont::vfs::fs_stat
Rust File: src/vfs/fs_stat.rs

---

## 1. Dependencies

From the source code and context, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `vfs::Error` enum. This type is used within the `Fail` struct to represent the specific error condition that occurred during the operation, mapping directly to NFSv3 status codes.
- **`super::file`**: Used to import `file::Attr` and `file::Handle`. `file::Handle` is used in the `Args` struct to identify the file system root being queried. `file::Attr` is used in both `Success` and `Fail` structs to return the attributes of the root handle, supporting the Weak Cache Consistency (WCC) mechanism of NFSv3.
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` attribute macro. This macro transforms the async `FsStat` trait into a version that is safe to use as a trait object (`dyn FsStat`) and is `Send`, allowing the trait to be used in multi-threaded, asynchronous contexts where dynamic dispatch is required.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `FSSTAT` procedure, which allows clients to query dynamic information about the file system, such as total and free space (bytes and file slots/inodes).
- To provide a structured return type that separates successful statistics retrieval from failure cases while preserving file attributes in both scenarios (to support cache consistency).

Inputs:
- **`args: Args`**: A structure containing a `file::Handle` (`root`) that identifies the file system or the specific mount point being queried.

Outputs:
- **`Result<Success, Fail>`**:
 - **`Success`**: Contains the file system statistics (`total_bytes`, `free_bytes`, `available_bytes`, `total_files`, `free_files`, `available_files`) and the `root_attr`.
 - **`Fail`**: Contains the `vfs::Error` describing the failure and the `root_attr` (if available).

Steps:
1. **Invocation**: The `fs_stat` method is called on an implementation of the `FsStat` trait with the provided `Args`.
2. **Resolution**: The implementation resolves the `file::Handle` to a specific file system instance.
3. **Querying**: The implementation queries the underlying storage system for capacity and usage metrics.
4. **Attribute Retrieval**: The implementation retrieves the current attributes (`file::Attr`) for the root handle.
5. **Response Construction**:
 - If successful, the implementation populates `Success` with the statistics and attributes.
 - If an error occurs (e.g., invalid handle, I/O error), the implementation populates `Fail` with the error and any attributes that could be retrieved.
6. **Return**: The `Result` is returned to the caller.

Edge Cases:
- **Stale Handle**: If the provided `file::Handle` is invalid or refers to a deleted file system, the method returns `Fail` with `vfs::Error::StaleFile`.
- **Attribute Availability**: The `root_attr` field in both `Success` and `Fail` is an `Option`. This allows the implementation to return statistics or errors even if the attributes for the root handle could not be retrieved, though the NFSv3 protocol generally expects attributes to be returned if the object exists.

Complexity:
- **Time**: O(1) conceptually, as `FSSTAT` is typically a metadata operation. The actual time depends on the underlying file system implementation.
- **Space**: O(1) for the structures defined.

Determinism:
- **Deterministic**: The output is strictly determined by the state of the file system at the moment of the call and the validity of the input handle.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: This module uses `Handle` as the input key (`Args::root`) to identify the target file system. The `Handle` serves as an opaque identifier that the implementation must map to a specific backend storage instance.
 - **`Attr`**: This module uses `Attr` in the return types (`Success::root_attr`, `Fail::root_attr`). This allows the client to update its cache for the root directory handle immediately after the `FSSTAT` operation, ensuring consistency between the client's view of the directory metadata and the server's actual state.

- **From `nfs_mamont::vfs`**:
 - **`Error`**: This module uses the `vfs::Error` enum to report failures. By using this centralized error type, the `FsStat` trait integrates seamlessly with the broader VFS error handling strategy, allowing errors like `IO` or `StaleFile` to be propagated correctly to the RPC layer.

---

## 4. Data Model

Entities:
- **`Args`**: Arguments for the `fs_stat` operation.
 - `root`: `file::Handle` — Identifies the file system root.
- **`Success`**: Successful result of the operation.
 - `root_attr`: `Option<file::Attr>` — Attributes of the root handle.
 - `total_bytes`: `u64` — Total capacity in bytes.
 - `free_bytes`: `u64` — Free space in bytes.
 - `available_bytes`: `u64` — Free space available to the user (excluding reserved space).
 - `total_files`: `u64` — Total file slots (inodes).
 - `free_files`: `u64` — Free file slots.
 - `available_files`: `u64` — Free file slots available to the user.
 - `invarsec`: `u32` — A measure of file system volatility (cache validity hint).
- **`Fail`**: Failed result of the operation.
 - `error`: `vfs::Error` — The specific error that occurred.
 - `root_attr`: `Option<file::Attr>` — Attributes of the root handle (if retrievable).

Relations:
- **`Args` → `file::Handle`**: Composition.
- **`Success` → `file::Attr`**: Optional Composition (0..1).
- **`Fail` → `vfs::Error`**: Composition.
- **`Fail` → `file::Attr`**: Optional Composition (0..1).

Global Invariants:
- **Space Hierarchy**: `available_bytes` must be less than or equal to `free_bytes`. `available_files` must be less than or equal to `free_files`. This reflects the distinction between "free" resources and resources available to non-privileged users (excluding reserved blocks/slots).

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type wrapped in the `Fail` struct. This enum covers standard NFSv3 errors (e.g., `IO`, `StaleFile`, `Access`).

Error Propagation Strategy:
- **Struct Wrapping**: Errors are propagated by returning the `Err` variant of `Result`, which contains the `Fail` struct. The `Fail` struct explicitly separates the error code from the object attributes, allowing the caller to access attributes even when the primary operation (getting stats) failed or partially failed.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` variant. For example, `IO` errors might be transient (network glitch, storage busy), while `StaleFile` indicates a persistent state issue requiring the client to perform a new lookup.

Panics:
- **Allowed**: No. The interface is defined to return `Result`, ensuring that all error conditions are handled via the `Fail` struct.

---

## 6. Traits

List which external traits this module implements:
- **`Send`**: Implemented for `FsStat` via the `#[trait_variant::make(Send)]` macro. This allows the trait object to be sent between threads.

List which traits this module defines:
- **`FsStat`**: The core trait defining the `fs_stat` asynchronous method.

---

## 7. Overview

This module is used in order to **implement the NFSv3 `FSSTAT` procedure interface**, which is essential for clients to monitor the health and capacity of the server's file system. The system contains a complex architecture where the Virtual File System (VFS) is abstracted behind traits to allow different storage backends. This module defines the specific contract for querying file system statistics.

A typical usage scenario of the system involves a client connecting to the NFS server and issuing an `FSSTAT` request to check how much disk space is available before attempting a large file upload. The RPC layer receives the request, extracts the file handle, and invokes the `fs_stat` method on the VFS implementation. The implementation queries the underlying OS or storage engine for `statvfs`-like information and returns it via the `Success` struct. The `invarsec` field is particularly important for client-side caching; it tells the client how long the returned statistics can be considered valid before they should be refreshed.

Inside the system, the following things happen and they use this module:
1. **Capacity Management**: The server uses this module to expose raw storage metrics. The distinction between `free_bytes` and `available_bytes` is critical here, as it allows the server to report space that is technically free but reserved for the root user or system processes, preventing non-privileged users from filling the disk entirely.
2. **Cache Consistency**: By including `root_attr` in both `Success` and `Fail`, the module supports the NFSv3 Weak Cache Consistency model. If the attributes of the root directory change (e.g., due to another client creating a file), the server can return the new attributes along with the stats, allowing the client to update its cache immediately without issuing a separate `GETATTR` request.
3. **Trait Aggregation**: The `FsStat` trait is aggregated into the main `Vfs` super-trait (defined in `vfs::mod.rs`). This means that any storage backend claiming to be a full `Vfs` implementation must provide logic to calculate and return these statistics, ensuring that all file systems exported by the server support this standard monitoring operation.

Without this module, the VFS interface would lack a standardized way to report file system usage, forcing clients to rely on outdated information or guess capacity, potentially leading to failed write operations or inefficient space usage.