<!-- SPEC_HASH: 62488809ef397575213269148fa9b0b47636a282ed64343a186399683064ef0a -->
# Module Specification

Module: mirrorfs::fs::rename_impl
Rust File: src/fs/rename_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs`**: Used to perform the asynchronous file system rename operation (`fs::rename`). This allows the implementation to move files or directories without blocking the Tokio runtime thread.
- **`nfs_mamont::vfs`**: Used to import the `rename` module trait and types (`rename::Args`, `rename::Success`, `rename::Fail`, `vfs::Error`, `vfs::WccData`). These define the contract that this implementation must satisfy, including the specific error codes and Weak Cache Consistency (WCC) data structures required by the NFSv3 protocol.
- **`super::MirrorFS`**: The struct for which the `rename` functionality is being implemented. The implementation relies on internal methods of `MirrorFS` (such as `path_for_handle`, `rename_cached_path`, etc.) to translate NFS handles into local filesystem paths and to maintain the internal cache consistency.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Argument Sanitization and Validation

**Intent:**
To enforce protocol-level constraints on the names used in the rename operation before any filesystem interaction occurs. This prevents invalid operations that could corrupt the filesystem state or violate NFS specifications.

**Inputs:**
- `args.from.name`: The source name.
- `args.to.name`: The target name.

**Outputs:**
- `Result<rename::Success, rename::Fail>`: Returns `Err(rename::Fail)` with `vfs::Error::InvalidArgument` if validation fails.

**Steps:**
1. Checks if `args.from.name` matches "." or "..".
2. Checks if `args.to.name` matches "." or "..".
3. If either check is true, returns an error immediately with empty `WccData` for both directories.

**Edge Cases:**
- **Early Exit**: If validation fails, the function returns immediately without resolving handles or accessing the disk, resulting in `None` for all WCC attributes.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 2: Handle-to-Path Resolution and Pre-Operation State Capture

**Intent:**
To translate the abstract NFS file handles (which identify directories) into concrete filesystem paths and capture the state of these directories *before* the modification. This is essential for providing accurate Weak Cache Consistency (WCC) data to the client.

**Inputs:**
- `args.from.dir`: Handle of the source directory.
- `args.to.dir`: Handle of the target directory.

**Outputs:**
- `PathBuf`: The concrete filesystem paths for the source and target directories.
- `Option<file::WccAttr>`: Metadata representing the state of the directories before the operation.

**Steps:**
1. Calls `self.path_for_handle(&args.from.dir)` to resolve the source directory path. If this fails, returns the error.
2. Calls `self.path_for_handle(&args.to.dir)` to resolve the target directory path. If this fails, returns the error.
3. Uses `std::fs::symlink_metadata` to read the metadata of both directory paths.
4. Converts this metadata into `WccAttr` using `Self::wcc_attr_from_metadata`.

**Edge Cases:**
- **Handle Invalidation**: If `path_for_handle` fails (e.g., stale handle), the operation aborts, and the error is propagated with empty WCC data (since the pre-state could not be read).

**Complexity:**
- Time: O(1) (assuming handle lookup is O(1)).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 3: Target Existence and Compatibility Check

**Intent:**
To enforce the NFSv3 rule that a rename cannot overwrite a target unless the source and target are compatible types (file-to-file or empty-directory-to-directory). This prevents data loss or type mismatches.

**Inputs:**
- `from_path`: The full path of the source object.
- `to_path`: The full path of the target object.

**Outputs:**
- `Result<(), rename::Fail>`: Returns an error if the target exists and is incompatible.

**Steps:**
1. Attempts to read metadata for `to_path` using `Self::metadata`.
2. If the target does not exist, proceeds to the rename step.
3. If the target exists:
 - Checks if `from_meta.is_dir()` matches `target_meta.is_dir()`. If not, returns `vfs::Error::Exist`.
 - If the target is a directory, checks if it is empty using `std::fs::read_dir(&to_path)`.
 - If the directory iterator yields any entry, returns `vfs::Error::Exist`.
 - If the target is a compatible, empty directory, calls `self.remove_cached_path(&to_path).await` to invalidate the cache for the path being overwritten.

**Edge Cases:**
- **Blocking I/O**: The check for an empty directory uses `std::fs::read_dir`, which is a synchronous (blocking) call. In an async context, this could potentially block the executor if the directory is on a slow or networked filesystem.

**Complexity:**
- Time: O(N) where N is the number of entries in the target directory (only if it is a directory).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 4: Atomic Rename and Cache Synchronization

**Intent:**
To perform the actual move operation on the filesystem and update the internal mapping of NFS handles to paths to reflect the new location of the file.

**Inputs:**
- `from_path`: The current path of the object.
- `to_path`: The desired new path of the object.

**Outputs:**
- `Result<rename::Success, rename::Fail>`: Returns success if the move and cache update succeed.

**Steps:**
1. Calls `fs::rename(&from_path, &to_path).await` to perform the asynchronous rename.
2. If the rename fails, converts the `io::Error` to `vfs::Error` and returns failure with WCC data.
3. If the rename succeeds, calls `self.rename_cached_path(&from_path, &to_path).await` to update the internal cache.
4. If the cache update fails, returns an error (note: the file is already moved on disk at this point, but the internal state is inconsistent).
5. On success, calculates the final `WccData` for both directories by re-reading their metadata and returns `Success`.

**Edge Cases:**
- **Cache Update Failure**: If `rename_cached_path` fails after a successful `fs::rename`, the function returns an error. The filesystem state is changed, but the server's internal cache might be stale or incorrect.
- **Cross-Device Moves**: The implementation relies on `fs::rename` to fail for cross-device moves. The error is then translated via `Self::io_error_to_vfs`, presumably mapping `EXDEV` to `vfs::Error::XDev`.

**Complexity:**
- Time: Dependent on the underlying filesystem (usually O(1) for metadata updates).
- Space: O(1).

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::rename`**:
 - **`Rename` Trait Contract**: The module implements the `rename` method defined by this trait. It adheres to the requirements of returning `WccData` for both source and target directories in both success and failure cases, and handling specific error conditions like `InvalidArgument` and `Exist`.
 - **`Args`, `Success`, `Fail`**: The module uses these structures to frame the input and output of the operation, ensuring compatibility with the generic VFS layer.

- **From `nfs_mamont::vfs`**:
 - **`Error` Enum**: The module uses specific variants like `InvalidArgument` and `Exist` to signal protocol-compliant failures. It also relies on the general `Error` type to wrap I/O errors encountered during path resolution or filesystem operations.
 - **`WccData`**: The module constructs `WccData` instances to report the state of directories before and after the operation, fulfilling the Weak Cache Consistency requirement of the NFS protocol.

- **From `mirrorfs::fs` (Assumptions based on usage)**:
 - **`path_for_handle`**: Used to resolve the abstract `Handle` into a concrete `PathBuf`. This is critical for bridging the NFS protocol with the local filesystem.
 - **`rename_cached_path` / `remove_cached_path`**: Used to maintain the internal consistency of `MirrorFS`. Since `MirrorFS` likely caches the mapping between handles and paths, these methods are necessary to update or invalidate that cache when the filesystem structure changes.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It operates on entities defined in `nfs_mamont::vfs::rename` and `nfs_mamont::vfs`.

Relations:
- **Implementation**: `MirrorFS` implements `nfs_mamont::vfs::rename::Rename`.

Global Invariants:
- **WCC Reporting**: The function guarantees that `WccData` is returned for both the source and target directories in all return paths (Success or Fail), except for early validation failures where the directories could not be resolved.
- **Atomicity**: The operation relies on the atomicity of the underlying `fs::rename` call. If `fs::rename` succeeds, the file is moved; if it fails, the state is unchanged (excluding potential cache update failures).

## 5. Error Model

Error Types:
- **`vfs::Error::InvalidArgument`**: Returned if the source or target name is "." or "..".
- **`vfs::Error::Exist`**: Returned if the target exists and is incompatible with the source (e.g., file vs. directory) or if the target is a non-empty directory.
- **`vfs::Error` (from `path_for_handle`)**: Returned if the directory handles are invalid or stale.
- **`vfs::Error` (from `fs::rename`)**: Returned if the underlying filesystem operation fails (e.g., I/O error, cross-device link).
- **`vfs::Error` (from `rename_cached_path`)**: Returned if the internal cache update fails after a successful rename.

Error Propagation Strategy:
- **Wrapper Struct**: All errors are wrapped in the `rename::Fail` struct, which includes the specific `vfs::Error` and the `WccData` for both directories.

Recoverability:
- **Client-Side**: Errors like `InvalidArgument` or `Exist` indicate client request errors and are not recoverable by retrying the same request. I/O errors might be transient.

Panics:
- **Allowed**: No explicit panics are present in the code. All error paths return `Result`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::rename::Rename`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete implementation of the NFSv3 RENAME procedure for the `MirrorFS` backend**. The system contains a Virtual File System (VFS) abstraction layer (`nfs_mamont::vfs`) that defines the interface for file operations like renaming, independent of the underlying storage. This module is necessary because it bridges the gap between that abstract protocol definition and the concrete reality of a local (or mirrored) filesystem managed by `MirrorFS`. It handles the translation of opaque NFS handles into filesystem paths, enforces protocol-specific constraints (like preventing renames to "."), and manages the side effects of the operation on the server's internal path cache.

A typical usage scenario of the system involves an NFS client requesting to move a file from one directory to another. The RPC layer decodes this request into `rename::Args` and invokes the `rename` method on the `MirrorFS` instance. This module then resolves the directory handles to local paths, checks if the target is an empty directory (if applicable), and performs the asynchronous rename. Crucially, it captures the state of the directories before and after the operation to construct `WccData`, which is returned to the client to ensure cache consistency across the network.

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: The module enforces rules that the underlying OS filesystem might not check in the same way (e.g., explicitly rejecting "." and ".." as names), ensuring the server adheres to the NFS specification.
2.  **Cache Coherence**: The module interacts with `MirrorFS`'s internal cache (`rename_cached_path`, `remove_cached_path`) to ensure that the server's mapping of file handles to paths remains accurate after the filesystem structure is modified. Without this, subsequent operations using the old paths would fail.
3.  **Error Translation**: The module converts raw OS I/O errors (e.g., from `fs::rename`) into standardized NFS error codes (e.g., `XDev` for cross-device attempts), providing a consistent error interface to the client regardless of the underlying OS.

**Uncertainty**: The provided facts for `mirrorfs::fs` do not list the methods `path_for_handle`, `remove_cached_path`, `rename_cached_path`, `wcc_attr_from_metadata`, `attr_from_metadata`, `wcc_data`, `io_error_to_vfs`, or `metadata`. The analysis assumes these are internal or private methods of `MirrorFS` (or defined in a private super-module) that handle the specifics of path resolution and caching, as they are extensively used in the implementation.