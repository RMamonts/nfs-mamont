<!-- SPEC_HASH: e966616f983f3215d6d1ddac3c44e41336edba15706110ed4e0c18a2980d38ed -->
# Module Specification

Module: mirrorfs::fs::remove_impl
Rust File: mirror_fs/src/fs/remove_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs`**: Used to perform the asynchronous file system operation `remove_file`. This allows the NFS server to delete files without blocking the main thread, which is crucial for high-performance asynchronous runtimes like Tokio.
- **`nfs_mamont::vfs`**: Used to import the `remove` trait and associated types (`remove::Args`, `remove::Success`, `remove::Fail`, `vfs::Error`, `vfs::WccData`). These define the contract that this implementation must satisfy to be compatible with the broader NFS server architecture.
- **`super::MirrorFS`**: The parent struct for which this implementation is defined. The module utilizes methods on `MirrorFS` (assumed to exist, such as `path_for_handle`, `child_path`, `remove_cached_path`) to translate abstract NFS file handles into concrete file system paths and manage internal caching.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `vfs::remove::Remove` for `MirrorFS`

**Intent:**
To provide the concrete logic for deleting a file within the `MirrorFS` backend. This implementation bridges the abstract NFS `REMOVE` procedure with the local file system, ensuring that protocol requirements (such as Weak Cache Consistency data) are met and safety checks (like preventing deletion of directories) are enforced.

**Inputs:**
- `args: remove::Args`: Contains the directory handle (`args.object.dir`) and the name of the entry to remove (`args.object.name`).

**Outputs:**
- `Result<remove::Success, remove::Fail>`:
 - `Ok(Success)`: Indicates the file was successfully deleted. Contains `wcc_data` reflecting the directory's state before and after the operation.
 - `Err(Fail)`: Indicates the operation failed. Contains the specific `vfs::Error` and `dir_wcc` (directory attributes, typically pre-operation).

**Steps:**
1. **Name Validation**: Calls `Self::ensure_name_allowed` with the entry name. If the name is invalid (e.g., contains illegal characters), it returns `Fail` immediately with empty WCC data.
2. **Directory Resolution**: Calls `self.path_for_handle` to convert the directory handle into a filesystem path. If this fails (e.g., stale handle), it returns `Fail` with empty WCC data.
3. **Pre-Operation Snapshot**: Captures the directory's metadata using `std::fs::symlink_metadata` to establish the "before" state for WCC.
4. **Child Path Resolution**: Calls `self.child_path` to construct the full path to the target entry. If this fails, it returns `Fail` with WCC data containing the "before" snapshot.
5. **Entry Metadata Check**: Calls `Self::metadata` on the child path. If this fails, it returns `Fail` with WCC data.
6. **Type Check**: Verifies that the target is not a directory. If `child_meta.is_dir()` is true, it returns `Fail` with `vfs::Error::IsDir` and WCC data.
7. **Deletion**: Calls `fs::remove_file` to delete the file asynchronously. If this I/O operation fails, it converts the error to `vfs::Error` and returns `Fail` with WCC data.
8. **Cache Invalidation**: Calls `self.remove_cached_path` to update the internal state of `MirrorFS`.
9. **Success Response**: Returns `Success` with `WccData` generated from the directory path and the "before" snapshot (which implies fetching the "after" snapshot internally).

**Edge Cases:**
- **Invalid Names**: Names rejected by `ensure_name_allowed` result in an early error without accessing the disk.
- **Stale Handles**: If the directory handle cannot be resolved to a path, the operation fails.
- **Directory Deletion Attempt**: The code explicitly checks `is_dir()` and returns `vfs::Error::IsDir` rather than attempting to delete a directory, adhering to the NFS specification that `REMOVE` is for files only (directories should use `RMDIR`).

**Complexity:**
- Time: O(1) relative to the size of the filesystem (assuming path resolution and metadata lookup are constant time or dependent on path depth).
- Space: O(1) (allocates path buffers and metadata structs).

**Determinism:**
- Deterministic. The outcome depends entirely on the state of the filesystem and the validity of the inputs.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::remove`**:
 - **`Remove` Trait**: This module implements the `remove` method defined by this trait. The trait dictates the input (`Args`) and output (`Result<Success, Fail>`) types, enforcing a standard interface that the NFS server can use regardless of the backend.
 - **`WccData` Requirement**: The trait requires that both `Success` and `Fail` carry `WccData`. This module adheres to this by capturing directory metadata before the operation and (implicitly) after, ensuring the client can synchronize its cache.

- **From `nfs_mamont::vfs`**:
 - **`vfs::Error`**: Used to report specific failure conditions. This module maps I/O errors to `vfs::Error` and uses `vfs::Error::IsDir` to handle the edge case where a user tries to remove a directory via the `REMOVE` procedure.

- **From `mirrorfs::fs`**:
 - **`MirrorFS` Context**: The implementation relies on the internal state of `MirrorFS` to resolve handles to paths (`path_for_handle`, `child_path`) and to manage internal caching (`remove_cached_path`). While the specific logic of these methods is defined in the parent module, this module consumes them to execute the NFS operation.

---

## 4. Data Model

Entities:
- This module defines no new public structs or enums. It operates entirely on types defined in `nfs_mamont::vfs` and `nfs_mamont::vfs::remove`.

Relations:
- **Implementation**: `MirrorFS` implements `nfs_mamont::vfs::remove::Remove`.

Global Invariants:
- **WCC Consistency**: The implementation ensures that `WccData` is always returned, even on failure, by capturing the directory state as early as possible in the execution flow.
- **Filesystem Safety**: The operation guarantees that it will not unlink a directory, preventing potential corruption of the directory tree structure.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned within `remove::Fail`. This module specifically generates `vfs::Error::IsDir` if the target is a directory and maps `std::io::Error` to appropriate `vfs::Error` variants via `Self::io_error_to_vfs`.

Error Propagation Strategy:
- **Early Return with Context**: The function uses a pattern of `match` or `if let Err` blocks. Upon encountering an error at any stage (name check, path resolution, metadata check, deletion), it immediately returns a `remove::Fail` struct. This struct wraps the error and includes the `dir_wcc` (Weak Cache Consistency data) captured up to that point.

Recoverability:
- **Dependent on Error Type**:
 - `vfs::Error::IsDir`: Not recoverable for this specific operation; the client must use `RMDIR` instead.
 - `vfs::Error::IO`: Potentially transient (e.g., disk full), though the client usually treats IO errors as fatal for the specific request.
 - `vfs::Error::Stale` (implied by handle resolution failure): The client must perform a new lookup to obtain a valid handle.

Panics:
- **Allowed**: Indirectly.
- **Conditions**:
 - If `tokio::fs::remove_file` panics (unlikely).
 - If the internal methods of `MirrorFS` (e.g., `path_for_handle`) panic.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::remove::Remove`**: Implemented for `MirrorFS`. This provides the asynchronous `remove` method required by the VFS layer.

---

## 7. Overview

This module is used in order to **provide the concrete implementation of the NFSv3 REMOVE procedure for the MirrorFS backend**. The system contains a complex architecture where the `nfs_mamont` crate defines the generic interfaces (traits) for file system operations, and `mirrorfs` provides a specific implementation backed by the local disk (or a mirror thereof). This module is necessary because the generic `Remove` trait only defines *what* a remove operation looks like (arguments and return values), whereas this module defines *how* it actually happens: resolving handles to paths, checking file types, and invoking the asynchronous file system deletion.

A typical usage scenario of the system involves an NFS client sending a REMOVE request for a file. The request is decoded by the RPC layer into `remove::Args`. The server then calls the `remove` method on the `MirrorFS` instance. This module executes the logic: it checks if the name is valid, finds the directory on disk, checks if the target is actually a file (not a directory), and then deletes it. Throughout this process, it captures directory attributes to return WCC data, ensuring the client's cache remains consistent.

Inside the system, the following things happen and they use this module:
1. **Handle Translation**: The module relies on `MirrorFS`'s internal mechanisms to translate opaque NFS file handles into concrete filesystem paths. This abstraction allows the NFS protocol to operate without knowing the actual storage layout.
2. **Protocol Enforcement**: The module enforces specific NFS semantics, such as returning `IsDir` when attempting to remove a directory. This ensures the backend behaves correctly according to the NFSv3 specification, even if the underlying OS filesystem might allow different operations.
3. **Async Integration**: By using `tokio::fs::remove_file`, the module integrates the deletion operation into the asynchronous runtime, preventing the server from blocking while the disk performs the unlink operation.

**Uncertainty**: The source code references several helper methods (`ensure_name_allowed`, `path_for_handle`, `child_path`, `metadata`, `wcc_attr_from_metadata`, `wcc_data`, `io_error_to_vfs`, `remove_cached_path`) which are not defined in this file. Based on the usage patterns (e.g., `self.path_for_handle` vs `Self::ensure_name_allowed`), it is assumed that `path_for_handle`, `child_path`, and `remove_cached_path` are instance methods of `MirrorFS` (likely defined in `mod.rs` or another impl block), while the others are private associated functions or utility methods within the `MirrorFS` implementation context. The exact logic for WCC calculation (specifically how the "after" attributes are obtained) is encapsulated within `Self::wcc_data`.