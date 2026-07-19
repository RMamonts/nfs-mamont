<!-- SPEC_HASH: 0adf0bf57cc7dd97b08e8b70a0725d0261bd119e154a168efcf07309d8e78dac -->
# Module Specification

Module: mirrorfs::fs::rm_dir_impl
Rust File: mirror_fs/src/fs/rm_dir_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::rm_dir`**: Used to import the `RmDir` trait and its associated types (`Args`, `Success`, `Fail`). This module provides the concrete implementation of this trait for the `MirrorFS` struct, defining how the `RMDIR` NFS procedure is executed on the local filesystem.
- **`nfs_mamont::vfs`**: Used to import the `Error` enum and `WccData` struct. These are used to construct error responses and to provide Weak Cache Consistency data (attributes before and after the operation) required by the NFSv3 protocol.
- **`super::MirrorFS`**: The parent struct for which this implementation block is defined. The implementation relies on internal helper methods of `MirrorFS` (e.g., `path_for_handle`, `child_path`, `remove_cached_path`) to abstract the details of handle-to-path resolution and cache management.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `RmDir` for `MirrorFS`

**Intent:**
To execute the logic required to remove a subdirectory on the filesystem backing `MirrorFS`, while strictly adhering to the NFSv3 protocol semantics regarding error handling and cache consistency (WCC).

**Inputs:**
- `args: rm_dir::Args`: Contains the `object` field of type `vfs::DirOpArgs`, which holds the parent directory's handle and the name of the directory to remove.

**Outputs:**
- `Result<rm_dir::Success, rm_dir::Fail>`:
 - `Ok(Success)`: Indicates successful removal. Contains `wcc_data` reflecting the parent directory's state after the operation.
 - `Err(Fail)`: Indicates failure. Contains the specific `vfs::Error` and `dir_wcc` reflecting the parent directory's state before the operation (if available).

**Steps:**
1. **Name Validation**: Checks if `args.object.name` is "." or "..". If either is found, the function immediately returns `Err(rm_dir::Fail)` with `vfs::Error::InvalidArgument` and empty WCC data.
2. **Parent Path Resolution**: Calls `self.path_for_handle(&args.object.dir).await` to convert the parent directory handle into a filesystem path. If this fails, the error is propagated with empty WCC data.
3. **Pre-Operation Metadata Capture**: Attempts to read the metadata of the parent directory using `std::fs::symlink_metadata`. This is stored as `before` for WCC calculation.
4. **Child Path Resolution**: Calls `self.child_path(&args.object.dir, &args.object.name).await` to construct the full path to the target directory. If this fails, the error is propagated with `dir_wcc` constructed from the `before` metadata.
5. **Child Metadata Verification**: Calls `Self::metadata(&child_path)` to retrieve metadata for the target. If this fails, the error is propagated with `dir_wcc`.
6. **Type Check**: Verifies that the target is actually a directory using `child_meta.is_dir()`. If it is not, returns `Err(rm_dir::Fail)` with `vfs::Error::NotDir` and `dir_wcc`.
7. **Removal**: Calls `std::fs::remove_dir(&child_path)` to delete the directory.
8. **Cache Invalidation**: On successful removal, calls `self.remove_cached_path(&child_path).await` to update the internal cache state of `MirrorFS`.
9. **Response Construction**: Returns `Ok(rm_dir::Success)` with `wcc_data` constructed from the `before` metadata and the current metadata of the parent directory.
10. **IO Error Handling**: If `std::fs::remove_dir` fails, the `std::io::Error` is converted to `vfs::Error` via `Self::io_error_to_vfs`, and `Err(rm_dir::Fail)` is returned with `dir_wcc`.

**Edge Cases:**
- **Special Entries**: "." and ".." are explicitly rejected with `InvalidArgument` before any filesystem access is attempted.
- **Non-Directory Target**: If the target exists but is a file, `NotDir` is returned.
- **Handle Resolution Failures**: Failures to resolve handles to paths result in errors with empty WCC data (since the path is unknown).

**Complexity:**
- **Time**: O(1) for name checks. O(N) for filesystem operations (metadata lookups, directory removal), where N depends on the underlying filesystem and directory size.
- **Space**: O(1) excluding the space required for path strings managed by the `MirrorFS` helpers.

**Determinism:**
- **Deterministic**: The logic flow is strictly determined by the input arguments and the state of the filesystem.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::rm_dir`**:
 - **`RmDir` Trait**: This module implements the `rm_dir` method defined by this trait. The trait dictates the signature, the use of `Args`, `Success`, and `Fail` structs, and the requirement to handle Weak Cache Consistency data.
 - **`Args`, `Success`, `Fail`**: These structs are the primary vehicles for data transfer. `Args` provides the target handle and name, while `Success` and `Fail` wrap the WCC data and error codes required by the protocol.

- **From `nfs_mamont::vfs`**:
 - **`Error` Enum**: This module uses specific variants like `InvalidArgument` and `NotDir` to signal protocol-compliant error conditions back to the client.
 - **`WccData`**: This module constructs `WccData` instances (via `Self::wcc_data`) to populate the `Success` and `Fail` structs, ensuring the client can validate its cache.

- **From `mirrorfs::fs::MirrorFS` (Assumed based on usage)**:
 - **`path_for_handle`**: Used to translate the abstract VFS handle into a concrete filesystem path.
 - **`child_path`**: Used to resolve the full path of the entry to be removed relative to its parent.
 - **`remove_cached_path`**: Used to maintain the internal consistency of the `MirrorFS` cache by removing the deleted directory's entry.
 - **`metadata`**: Used to inspect the filesystem node type (directory vs. file).
 - **`io_error_to_vfs`**: Used to translate standard OS errors into the VFS error domain.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates entirely on the types defined in `nfs_mamont::vfs::rm_dir` and `nfs_mamont::vfs`.

Relations:
- **`MirrorFS` implements `RmDir`**: This module establishes the implementation relationship between the concrete filesystem struct and the generic VFS trait.

Global Invariants:
- **WCC Reporting**: If the parent directory path is successfully resolved, `dir_wcc` in the `Fail` case must contain the `before` attributes. In the `Success` case, `wcc_data` must contain both `before` and `after` attributes.
- **Special Entry Rejection**: The operation must never attempt to resolve a path for "." or ".." on the filesystem; these must be rejected immediately at the argument validation stage.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type returned within `rm_dir::Fail`.
 - **`InvalidArgument`**: Returned if the target name is "." or "..".
 - **`NotDir`**: Returned if the target path exists but is not a directory.
 - **IO / others**: Returned if filesystem operations (metadata lookup, removal) fail, mapped via `io_error_to_vfs`.

Error Propagation Strategy:
- **Early Return**: The function uses early returns (`Err(rm_dir::Fail)`) upon encountering validation errors or failures in helper methods (`path_for_handle`, `child_path`).
- **WCC Preservation**: When an error occurs after the parent path is resolved, the `before` metadata is preserved and returned in `dir_wcc` to allow the client to update its cache despite the failure.

Recoverability:
- **Dependent on Error**: Transient errors (like `IO`) might be recoverable by the client retrying. Logical errors (like `InvalidArgument` or `NotDir`) are permanent for the specific request context.

Panics:
- **Allowed**: No. The implementation is designed to handle all error conditions via `Result`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::rm_dir::RmDir`**: Implemented for `mirrorfs::fs::MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete filesystem logic for the NFSv3 RMDIR operation within the `MirrorFS` backend**. The system contains a layered architecture where the top layer (`nfs_mamont`) defines generic protocol interfaces (traits), and the bottom layer (`mirrorfs`) provides the actual storage implementation. This module is necessary because it bridges the gap between the abstract "remove directory" command defined by the NFS protocol and the specific mechanics of the `MirrorFS` implementation (handle resolution, path manipulation, and cache management).

A typical usage scenario of the system involves an NFS client sending a RMDIR request. The request is decoded by the RPC layer and dispatched to the `MirrorFS` instance via the `Vfs` super-trait. The `rm_dir` method in this module is invoked. It validates the input, translates the opaque file handles into real filesystem paths, checks that the target is indeed a directory, and then calls the standard library to delete it. Simultaneously, it gathers metadata before and after the operation to construct the Weak Cache Consistency (WCC) data, which is sent back to the client to ensure its directory cache remains valid. Finally, it updates the internal `MirrorFS` cache to reflect the deletion.

Inside the system, the following things happen and they use this module:
1. **Protocol Enforcement**: The module enforces specific constraints required by the NFS protocol, such as forbidding the deletion of "." and ".." by returning `InvalidArgument`.
2. **State Synchronization**: By calling `remove_cached_path`, the module ensures that the `MirrorFS` internal state (which likely caches inodes or handles) is synchronized with the actual filesystem state after a deletion.
3. **Error Translation**: The module converts low-level OS errors (e.g., from `std::fs::remove_dir`) into high-level NFS status codes (`vfs::Error`), ensuring that the client receives meaningful error information consistent with the NFS specification.

The critical aspect of this module is the **integration of cache consistency with filesystem mutation**. It does not merely delete a directory; it ensures that every step of the process—from input validation to post-operation metadata collection—is performed to maintain the integrity and consistency guarantees of the NFSv3 protocol.