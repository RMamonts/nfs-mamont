<!-- SPEC_HASH: e966616f983f3215d6d1ddac3c44e41336edba15706110ed4e0c18a2980d38ed -->
# Module Specification

Module: mirrorfs::fs::remove_impl
Rust File: src/fs/remove_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs`**: Used to perform asynchronous file system operations. Specifically, `fs::remove_file` is called to delete the target file from the underlying storage.
- **`nfs_mamont::vfs`**: Used to import the `remove` trait and associated types (`Args`, `Success`, `Fail`, `Error`, `WccData`). This module implements the `remove::Remove` trait for `MirrorFS`, adhering to the VFS interface contract.
- **`super::MirrorFS`**: The struct for which the `remove` functionality is implemented. The implementation utilizes internal methods of `MirrorFS` (e.g., `path_for_handle`, `child_path`, `remove_cached_path`) to handle path resolution and cache management.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `remove::Remove` for `MirrorFS`

**Intent:**
To provide the concrete logic for deleting a file within the `MirrorFS` backend while satisfying the NFSv3 protocol requirements, specifically regarding Weak Cache Consistency (WCC) and error handling.

**Inputs:**
- `args: remove::Args`: Contains the directory handle (`args.object.dir`) and the name of the entry to remove (`args.object.name`).

**Outputs:**
- `Result<remove::Success, remove::Fail>`:
  - `Ok(Success)`: Indicates the file was successfully removed. Contains `wcc_data` representing the directory state before and after the operation.
  - `Err(Fail)`: Indicates the operation failed. Contains the specific `vfs::Error` and `dir_wcc` (directory state, usually pre-operation).

**Steps:**
1. **Name Validation**: Calls `Self::ensure_name_allowed` with the entry name. If validation fails, returns `Fail` with `WccData` set to `None` for both before and after attributes.
2. **Directory Path Resolution**: Calls `self.path_for_handle(&args.object.dir).await` to convert the directory handle into a filesystem path. If this fails, returns `Fail` with empty `WccData`.
3. **Pre-Operation Snapshot**: Calls `std::fs::symlink_metadata(&dir_path)` to capture the directory's attributes *before* the modification. This is stored as `before`.
4. **Child Path Resolution**: Calls `self.child_path(&args.object.dir, &args.object.name).await` to construct the full path to the target entry. If this fails, returns `Fail` with `WccData` constructed from the `before` snapshot.
5. **Child Metadata Check**: Calls `Self::metadata(&child_path)` to retrieve the target's metadata. If this fails, returns `Fail` with `WccData`.
6. **Type Enforcement**: Checks if the target is a directory using `child_meta.is_dir()`. If true, returns `Fail` with `vfs::Error::IsDir` and `WccData`. (NFS `REMOVE` is strictly for files; directories require `RMDIR`).
7. **File Deletion**: Calls `fs::remove_file(&child_path).await` to delete the file. If an IO error occurs, it is converted to `vfs::Error` via `Self::io_error_to_vfs`, and `Fail` is returned with `WccData`.
8. **Cache Invalidation**: Calls `self.remove_cached_path(&child_path).await` to update internal `MirrorFS` caches.
9. **Success Response**: Returns `Success` with `WccData` constructed from the `before` snapshot and the current directory state (fetched internally by `Self::wcc_data`).

**Edge Cases:**
- **Invalid Name**: If the filename is rejected by `ensure_name_allowed`, the operation fails immediately without accessing the disk.
- **Target is Directory**: The implementation explicitly prevents removing directories, returning `vfs::Error::IsDir`.
- **Handle Resolution Failure**: If the directory handle cannot be resolved to a path, the operation fails with empty WCC data (since the directory is unknown).

**Complexity:**
- Time: O(1) relative to the number of files, assuming path resolution and metadata operations are O(1) or O(path length).
- Space: O(1).

**Determinism:**
- Deterministic. The outcome depends solely on the state of the filesystem and the input arguments.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::remove`**:
  - **`Remove` Trait**: The module implements this trait. The trait dictates the function signature (`async fn remove`) and the structure of `Args`, `Success`, and `Fail`.
  - **WCC Requirement**: The module adheres to the trait's implicit requirement that `Success` and `Fail` must carry `WccData` to ensure cache coherency for the client.

- **From `nfs_mamont::vfs`**:
  - **`vfs::Error`**: Used to report specific failure conditions, such as `IsDir` when attempting to remove a directory, or `IO` errors during file deletion.
  - **`vfs::WccData`**: Used to wrap the directory attributes captured before and after the operation.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on types defined in `nfs_mamont::vfs` and `nfs_mamont::vfs::remove`.

Relations:
- **Implementation**: `MirrorFS` implements `remove::Remove`.

Global Invariants:
- **Atomicity of WCC**: The `before` snapshot is taken strictly before any modification attempt (deletion). The `after` state is captured after the operation (or on failure, usually just `before` is provided).
- **Non-Directory Constraint**: The operation guarantees that it will not remove a directory; if the target is a directory, it returns an error instead of modifying the filesystem.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `remove::Fail`. Common variants returned include:
  - `IsDir`: If the target object is a directory.
  - `IO`: If the underlying `fs::remove_file` operation fails.
  - `NoEntry`: If path resolution or metadata lookup fails (implied by `MirrorFS` internal logic).

Error Propagation Strategy:
- **Early Return**: The function uses a series of `match` statements. If any step fails (validation, path resolution, type check, deletion), it immediately returns a `remove::Fail` struct containing the error and the available `WccData`.

Recoverability:
- **Dependent on Error**: `IO` errors might be transient (e.g., resource temporarily unavailable). `IsDir` or `NoEntry` are permanent failures for the specific request context.

Panics:
- **Allowed**: No explicit panics in this module. Panics would only arise from unwinding errors in the `MirrorFS` helper methods or Tokio runtime failures.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::remove::Remove`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **implement the NFSv3 REMOVE procedure for the MirrorFS backend**. It translates the high-level request to delete a file (defined by a directory handle and a filename) into specific filesystem actions: resolving handles to paths, verifying the target type, deleting the file, and managing cache consistency.

The system contains a complex Virtual File System (VFS) abstraction layer where storage backends must implement specific traits for each NFS operation. This module is necessary because the generic `nfs_mamont::vfs::remove::Remove` trait only defines the *interface*; it does not provide the logic for interacting with the actual disk or the specific path-resolution logic of `MirrorFS`. This module bridges that gap.

A typical usage scenario of the system involves an NFS client sending a REMOVE request for a file. The request is routed to the `MirrorFS` instance. The `remove` method in this module is called. It first ensures the name is valid, then translates the opaque directory handle into a concrete path on the local disk. It checks if the target is actually a file (rejecting directories). If valid, it uses `tokio::fs` to delete the file and updates the internal `MirrorFS` cache to reflect the removal. Finally, it returns the directory attributes before and after the operation to the client, allowing the client to invalidate its cache for that directory.

Inside the system, the following things happen and they use this module:
1. **Protocol Enforcement**: The module enforces the constraint that the REMOVE operation cannot be used to delete directories, returning `vfs::Error::IsDir` if the target is a directory. This ensures strict compliance with NFSv3 semantics, which separates file removal (`REMOVE`) from directory removal (`RMDIR`).
2. **Cache Coherency**: By capturing directory metadata before and after the deletion, the module provides the data required for the Weak Cache Consistency (WCC) mechanism. This is critical for the overall system because it allows multiple NFS clients to maintain consistent views of the filesystem without excessive polling.
3. **Backend Integration**: The module integrates with the internal state of `MirrorFS` by calling `remove_cached_path`. This ensures that the `MirrorFS` in-memory cache (which likely maps handles to paths or attributes) does not contain stale data after a file is deleted.

**Uncertainty**: The source code references several methods of `MirrorFS` (e.g., `ensure_name_allowed`, `path_for_handle`, `child_path`, `remove_cached_path`, `metadata`, `wcc_attr_from_metadata`, `wcc_data`, `io_error_to_vfs`) which are not listed in the provided public interface facts for `MirrorFS`. The specification assumes these are internal helper methods or private methods essential for the implementation of the VFS traits.