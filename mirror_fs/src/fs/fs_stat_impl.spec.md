<!-- SPEC_HASH: bc367b2a9746bb66e15432e586630078ad1a6ca8aa871803af2c846df9af328a -->
# Module Specification

Module: mirrorfs::fs::fs_stat_impl
Rust File: mirror_fs/src/fs/fs_stat_impl.rs

---

## 1. Dependencies

From the source code and context, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::fs_stat`**: Used to import the `FsStat` trait and the associated types `Args`, `Success`, and `Fail`. This module implements this trait for `MirrorFS`, allowing the `MirrorFS` struct to respond to NFSv3 `FSSTAT` procedure requests.
- **`super::MirrorFS`**: The parent struct for which the `FsStat` trait is being implemented. The implementation relies on methods `path_for_handle` and `file_attr` (associated function) defined on `MirrorFS` to perform the actual logic of resolving handles and retrieving file attributes.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a concrete implementation of the NFSv3 `FSSTAT` procedure for the `MirrorFS` backend. This allows the NFS server to report file system attributes and usage statistics for a mirrored file system, although the usage statistics are currently hardcoded to zero.

Inputs:
- **`args: fs_stat::Args`**: A structure containing a `file::Handle` (`root`) identifying the file system root or mount point to be queried.

Outputs:
- **`Result<fs_stat::Success, fs_stat::Fail>`**:
 - **`Success`**: Contains the attributes of the root handle and file system statistics.
 - **`Fail`**: Contains an error if the handle could not be resolved.

Steps:
1. **Handle Resolution**: The method calls `self.path_for_handle(&args.root).await` to convert the opaque NFS file handle into a concrete filesystem path.
2. **Error Handling**: If `path_for_handle` returns an error, the method immediately returns `fs_stat::Fail` containing that error and `root_attr: None`.
3. **Attribute Retrieval**: If the handle is resolved successfully, the method calls `Self::file_attr(&path)` to retrieve the current attributes for the resolved path.
4. **Response Construction**: The method constructs and returns a `fs_stat::Success` struct.
 - The `root_attr` field is populated with the attributes retrieved in the previous step.
 - All statistics fields (`total_bytes`, `free_bytes`, `available_bytes`, `total_files`, `free_files`, `available_files`, `invarsec`) are hardcoded to `0`.

Edge Cases:
- **Invalid Handle**: If the provided `root` handle cannot be resolved to a path by `path_for_handle`, the operation fails, and the error is propagated to the caller.

Complexity:
- **Time**: O(P) where P is the complexity of `path_for_handle` and `file_attr` (typically filesystem lookup time).
- **Space**: O(1).

Determinism:
- **Deterministic**: The output is determined by the result of `path_for_handle` and `file_attr`. The statistics are constant (zero).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::fs_stat`**:
 - **`FsStat` Trait**: This module implements the `fs_stat` method defined by this trait. The trait defines the contract for returning file system statistics and attributes.
 - **`Args`, `Success`, `Fail`**: These types structure the input and output of the operation. `Success` requires `root_attr` and various usage counters; `Fail` requires an error code and optional `root_attr`.

- **From `mirrorfs::fs` (Assumed)**:
 - **`path_for_handle`**: This mechanism is used to translate the abstract `file::Handle` into a concrete path usable by the underlying OS. *Uncertainty: The provided facts.json for `mirrorfs::fs` does not list `path_for_handle`, but the code explicitly invokes `self.path_for_handle(...).await`.*
 - **`file_attr`**: This associated function is used to retrieve the metadata (attributes) for a given path. *Uncertainty: The provided facts.json for `mirrorfs::fs` does not list `file_attr`, but the code explicitly invokes `Self::file_attr(...)`.*

---

## 4. Data Model

Entities:
- This module defines no new public entities. It implements the `FsStat` trait for the existing `MirrorFS` struct.

Relations:
- **`MirrorFS` implements `FsStat`**: Implementation relationship.

Global Invariants:
- **Zero Statistics**: The implementation enforces an invariant where all file system usage statistics (bytes and files) are reported as zero. This suggests that `MirrorFS` does not currently support dynamic capacity reporting or acts as a pass-through where capacity is unknown or irrelevant to the specific logic implemented here.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `fs_stat::Fail`. This error originates from the `path_for_handle` method of `MirrorFS`.

Error Propagation Strategy:
- **Early Return**: If `path_for_handle` returns an error, the method immediately wraps it in `fs_stat::Fail` and returns, preventing further execution.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` returned by `path_for_handle` (e.g., `StaleFile` vs `IO`).

Panics:
- **Allowed**: No. The implementation uses `match` and `Result` to handle errors explicitly.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::fs_stat::FsStat`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **enable the `MirrorFS` backend to respond to NFSv3 `FSSTAT` requests**. The system contains a complex architecture where storage backends must implement the `Vfs` super-trait to be usable by the NFS server. The `Vfs` trait includes `FsStat` as a required component. This module provides that specific implementation for `MirrorFS`.

A typical usage scenario of the system involves an NFS client querying the server to check the available disk space on a mounted share provided by `MirrorFS`. The server receives the request, identifies the target backend as `MirrorFS`, and invokes the `fs_stat` method implemented in this module. The module attempts to resolve the file handle to a local path. If successful, it retrieves the file attributes (like size and modification time) to return to the client for cache consistency. However, regarding capacity metrics (total/free space), the module returns zeros.

Inside the system, the following things happen and they use this module:
1.  **Protocol Compliance**: The NFSv3 protocol strictly requires the server to handle the `FSSTAT` procedure. Without this implementation, `MirrorFS` could not satisfy the `Vfs` trait bounds, and the server would fail to compile or fail to export the filesystem.
2.  **Attribute Synchronization**: While it reports zero capacity, the module ensures that the `root_attr` (attributes of the root directory) is correctly calculated and returned. This is crucial for the client to maintain cache consistency (Weak Cache Consistency) for the root directory, even if the client cannot determine disk usage.
3.  **Stub Implementation**: The hardcoding of statistics to zero indicates that `MirrorFS` is likely a specific type of backend (perhaps a mirror or a cache) where global capacity limits are not enforced or are managed at a different layer (e.g., the underlying source filesystem). The module serves as a placeholder that satisfies the interface contract without providing meaningful capacity data.

**Uncertainty**: The code relies on `self.path_for_handle` and `Self::file_attr`. The provided dependency facts for `mirrorfs::fs` do not list these methods, only `handle_for_path`. It is assumed that `path_for_handle` is the inverse operation of `handle_for_path` and exists on the `MirrorFS` struct, and that `file_attr` is a helper to get file metadata.