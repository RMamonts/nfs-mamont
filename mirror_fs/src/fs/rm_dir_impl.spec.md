<!-- SPEC_HASH: 0adf0bf57cc7dd97b08e8b70a0725d0261bd119e154a168efcf07309d8e78dac -->
# Module Specification

Module: mirrorfs::fs::rm_dir_impl
Rust File: mirror_fs/src/fs/rm_dir_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs`**: Used to import the `Error` enum (e.g., `InvalidArgument`, `NotDir`) and the `WccData` structure. These are required to construct the return types (`Success` and `Fail`) that conform to the NFSv3 protocol specification.
- **`nfs_mamont::vfs::rm_dir`**: Used to import the `RmDir` trait and its associated types (`Args`, `Success`, `Fail`). This module provides the concrete implementation of this trait for the `MirrorFS` struct.
- **`super::MirrorFS`**: The parent struct for which this implementation is defined. The implementation relies on several internal helper methods of `MirrorFS` (e.g., `path_for_handle`, `child_path`, `remove_cached_path`, `wcc_attr_from_metadata`, `wcc_data`, `io_error_to_vfs`) which are assumed to exist based on their usage in the code.
- **`std::fs`**: Used to perform synchronous filesystem operations such as `symlink_metadata` (to retrieve attributes for WCC) and `remove_dir` (to delete the directory).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Special Directory Entry Validation

**Intent:**
To enforce NFSv3 protocol restrictions regarding the removal of the current directory (`.`) and parent directory (`..`) entries. The protocol dictates that these specific names must result in distinct error codes rather than being treated as standard deletion requests.

**Inputs:**
- `args.object.name`: The name of the entry to be removed, provided as a string slice.

**Outputs:**
- `Err(rm_dir::Fail)`: A failure result containing `vfs::Error::InvalidArgument` and empty WCC data.

**Steps:**
1. Check if `args.object.name` is equal to `"."`.
2. If true, return `Err(rm_dir::Fail { error: vfs::Error::InvalidArgument, ... })`.
3. Check if `args.object.name` is equal to `".."`.
4. If true, return `Err(rm_dir::Fail { error: vfs::Error::InvalidArgument, ... })`.

**Edge Cases:**
- The code returns `InvalidArgument` for both `.` and `..`. The `nfs_mamont::vfs::rm_dir` specification suggests `..` *should* return `Exist`, but this implementation strictly returns `InvalidArgument` for both.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 2: Path Resolution and Weak Cache Consistency (WCC) Preparation

**Intent:**
To translate the abstract file handles used by the NFS protocol into concrete filesystem paths, and to capture the pre-operation state of the parent directory. This is necessary to perform the actual deletion and to provide the client with the data required to validate its cache.

**Inputs:**
- `args.object.dir`: The handle of the parent directory.
- `args.object.name`: The name of the child directory.

**Outputs:**
- `dir_path`: The filesystem path of the parent directory.
- `child_path`: The filesystem path of the child directory to be removed.
- `before`: An optional `WccAttr` representing the state of the parent directory before the operation.

**Steps:**
1. Call `self.path_for_handle(&args.object.dir).await` to resolve the parent handle to a path. If this fails, return an error immediately (no WCC data is available).
2. Call `std::fs::symlink_metadata(&dir_path)` to get the parent's metadata. Convert this to `WccAttr` using `Self::wcc_attr_from_metadata`.
3. Call `self.child_path(&args.object.dir, &args.object.name).await` to construct the full path of the target. If this fails, return an error including the `before` WCC data.

**Edge Cases:**
- If `path_for_handle` fails, `before` is `None`, so the returned `Fail` struct contains empty WCC data.
- If `child_path` fails, `before` is available, so the returned `Fail` struct contains the pre-operation attributes of the parent.

**Complexity:**
- Time: Depends on the internal implementation of `path_for_handle` and `child_path` (likely filesystem lookups).
- Space: O(1) for the paths/metadata structures.

**Determinism:**
- Deterministic (assuming the underlying `MirrorFS` helpers are deterministic).

### Mechanism 3: Directory Type Verification and Removal

**Intent:**
To ensure the target object is actually a directory before attempting to remove it, and then to perform the removal while updating the internal cache of the `MirrorFS`.

**Inputs:**
- `child_path`: The path to the target object.
- `dir_path`: The path to the parent directory.
- `before`: The pre-operation metadata of the parent.

**Outputs:**
- `Ok(rm_dir::Success)`: If removal succeeds.
- `Err(rm_dir::Fail)`: If the target is not a directory or if the filesystem removal fails.

**Steps:**
1. Call `Self::metadata(&child_path)` to retrieve the target's metadata. If this fails, return an error with parent WCC data.
2. Check `child_meta.is_dir()`. If false, return `Err(rm_dir::Fail { error: vfs::Error::NotDir, ... })`.
3. Call `std::fs::remove_dir(&child_path)`.
4. If `Ok(())`:
    - Call `self.remove_cached_path(&child_path).await` to invalidate internal caches.
    - Call `Self::wcc_data(&dir_path, before)` to generate the final WCC structure (likely fetching post-op attributes).
    - Return `Ok(rm_dir::Success { wcc_data: ... })`.
5. If `Err(io_error)`:
    - Convert the IO error to a `vfs::Error` using `Self::io_error_to_vfs`.
    - Return `Err(rm_dir::Fail { error: ..., dir_wcc: ... })`.

**Edge Cases:**
- `std::fs::remove_dir` will fail if the directory is not empty (OS level), which is then converted to a `vfs::Error`.

**Complexity:**
- Time: O(1) for the check, O(N) for the removal (where N is the number of entries, though `remove_dir` typically fails if not empty rather than iterating).
- Space: O(1).

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::rm_dir`**:
    - **`RmDir` Trait**: This module implements the `rm_dir` method defined by this trait. It adheres to the signature `async fn rm_dir(&self, args: Args) -> Result<Success, Fail>`.
    - **`Args`, `Success`, `Fail`**: These structures define the input and output contracts. The implementation populates `Success` with `wcc_data` and `Fail` with `error` and `dir_wcc` as required by the trait.

- **From `nfs_mamont::vfs`**:
    - **`Error` Enum**: The implementation uses specific variants like `InvalidArgument` and `NotDir` to signal protocol-level failures.
    - **`WccData`**: The implementation constructs this structure (via `Self::wcc_data`) to satisfy the NFSv3 Weak Cache Consistency requirement, ensuring the client can verify the parent directory's state.

- **From `mirrorfs::fs` (Assumptions based on usage)**:
    - **`MirrorFS` Internal State**: The implementation relies on `MirrorFS` maintaining a mapping between handles and paths (`path_for_handle`), constructing child paths (`child_path`), and managing an internal cache (`remove_cached_path`). These are not defined in the provided public interface of `MirrorFS` but are critical for the logic of this implementation.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on entities defined in `nfs_mamont::vfs::rm_dir` and `nfs_mamont::vfs`.

Relations:
- **Implementation Relation**: `MirrorFS` implements `nfs_mamont::vfs::rm_dir::RmDir`.

Global Invariants:
- **WCC Data Consistency**: If the parent directory path is successfully resolved, the `before` attribute in the `Fail` case must reflect the state of the directory prior to the attempted removal.
- **Cache Invalidation**: Upon successful removal of a directory, `remove_cached_path` must be called to ensure subsequent operations do not rely on stale cached data for that path.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `rm_dir::Fail`.
    - `InvalidArgument`: Returned if the target name is "." or "..".
    - `NotDir`: Returned if the target path exists but is not a directory.
    - `IO` / others: Returned via `io_error_to_vfs` if `std::fs::remove_dir` fails (e.g., directory not empty, permission denied).

Error Propagation Strategy:
- **Early Return**: The function uses a series of `match` or `?`-like logic (explicit `return Err`) to propagate failures immediately.
- **WCC Preservation**: When an error occurs after the parent directory metadata is successfully retrieved, the error payload includes the `before` WCC data. If the error occurs before that (e.g., invalid handle), the WCC data is empty (`None`).

Recoverability:
- **Dependent on Error**: `IO` errors might be transient (e.g., `JUKEBOX` equivalent), but `InvalidArgument` or `NotDir` are permanent client errors.

Panics:
- **Allowed**: No explicit panics are present in the code. All error paths return `Result`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::rm_dir::RmDir`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete implementation of the NFSv3 RMDIR procedure for the MirrorFS backend**. The system contains a layered architecture where the top-level `nfs_mamont` server handles the network protocol and dispatches requests to a Virtual File System (VFS) trait. The `MirrorFS` struct acts as a specific backend that mirrors a local filesystem. This module is necessary because it bridges the gap between the abstract VFS operations (defined by `nfs_mamont::vfs::rm_dir`) and the concrete OS filesystem operations required to delete a directory.

A typical usage scenario of the system involves an NFS client sending an RMDIR request for a specific directory. The request travels through the RPC layer to the `MirrorFS` instance. The `rm_dir` method in this module is invoked. It first validates the request (checking for "." and ".."), then resolves the abstract file handles to actual file paths on the disk. It captures the parent directory's metadata for cache consistency (WCC), verifies the target is indeed a directory, and then calls `std::fs::remove_dir`. Finally, it updates the internal cache of `MirrorFS` and returns the result to the client, including the WCC data to keep the client's cache synchronized.

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: The module enforces specific NFSv3 rules, such as rejecting attempts to remove "." or ".." with `InvalidArgument`. This ensures the backend behaves strictly according to the protocol specification.
2.  **Cache Coherence**: By capturing the parent directory's attributes before the operation and returning them (along with post-op attributes on success) via `WccData`, the module enables the client to validate its cache without re-reading the directory, which is crucial for NFS performance.
3.  **Internal State Management**: The module interacts with `MirrorFS`'s internal cache via `remove_cached_path`. This ensures that if a directory is deleted, any cached handles or path mappings associated with it are purged, preventing future operations from accessing a non-existent resource.

**Uncertainty**: The `MirrorFS` type definition provided in the dependency facts does not list the methods `path_for_handle`, `child_path`, `remove_cached_path`, `wcc_attr_from_metadata`, `wcc_data`, or `io_error_to_vfs`. The analysis assumes these methods exist on `MirrorFS` or are inherent `impl` blocks associated with it, as they are called as `self.method(...)` or `Self::method(...)` in the source code. Their behavior is inferred from their names and usage context.