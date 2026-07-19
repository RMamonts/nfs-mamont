<!-- SPEC_HASH: 62488809ef397575213269148fa9b0b47636a282ed64343a186399683064ef0a -->
# Module Specification

Module: mirrorfs::fs::rename_impl
Rust File: src/fs/rename_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs`**: Used to perform the asynchronous file system operation `rename`. This module relies on Tokio's non-blocking filesystem interface to execute the actual move/rename operation on the underlying storage.
- **`nfs_mamont::vfs`**: Used to import the `rename` module (which defines the `Rename` trait and associated types `Args`, `Success`, `Fail`) and core types like `Error` and `WccData`. These are necessary to implement the VFS contract and return protocol-compliant results.
- **`super::MirrorFS`**: The struct for which the `Rename` trait is implemented. The implementation uses internal methods of `MirrorFS` (e.g., `path_for_handle`, `remove_cached_path`, `rename_cached_path`) to translate abstract file handles into concrete filesystem paths and to manage internal cache consistency.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `vfs::rename::Rename` for `MirrorFS`

**Intent:**
To provide the concrete logic for renaming file system objects within the `MirrorFS` backend, ensuring compliance with NFSv3 semantics regarding atomicity, error conditions, and Weak Cache Consistency (WCC) data reporting.

**Inputs:**
- `args: rename::Args`: Contains `from` (source directory handle and name) and `to` (target directory handle and name).

**Outputs:**
- `Result<rename::Success, rename::Fail>`: Returns a structure containing `WccData` for both the source and target directories in both success and failure cases.

**Steps:**
1. **Name Validation**: Checks if `args.from.name` or `args.to.name` are "." or "..". If so, returns `Fail` with `vfs::Error::InvalidArgument` and empty WCC data.
2. **Path Resolution**: Calls `self.path_for_handle` asynchronously for both source and target directory handles. If resolution fails, returns `Fail` with the specific error and empty WCC data.
3. **Pre-Operation Metadata Capture**: Retrieves `symlink_metadata` for both source and target directories using `std::fs::symlink_metadata`. This data is converted to `WccAttr` and `Attr` to form the `before` state of the `WccData`.
4. **Path Construction**: Constructs full absolute paths for the source object (`from_path`) and target object (`to_path`) by joining the directory paths with the entry names.
5. **Identity Check**: Compares `from_path` and `to_path`. If they are identical, returns `Success` immediately using the pre-captured metadata for both WCC structures (no-op).
6. **Source Verification**: Calls `Self::metadata` on `from_path`. If the source does not exist or is inaccessible, returns `Fail`.
7. **Target Conflict Resolution**:
   - Checks if `to_path` exists via `Self::metadata`.
   - If it exists, checks type compatibility (directory vs. non-directory). If incompatible, returns `Fail` with `vfs::Error::Exist`.
   - If the target is a directory, checks if it is empty using `std::fs::read_dir`. If it contains entries, returns `Fail` with `vfs::Error::Exist`.
   - If the target is compatible and (if a directory) empty, calls `self.remove_cached_path(&to_path).await` to invalidate internal cache entries for the target.
8. **Execution**: Invokes `fs::rename(&from_path, &to_path).await` to perform the asynchronous rename operation.
9. **Cache Update**: On success, calls `self.rename_cached_path(&from_path, &to_path).await` to update internal path mappings.
10. **Post-Operation WCC Generation**: Calls `Self::wcc_data` for both directories to capture the `after` state (which involves re-reading metadata) and returns `Success`.

**Edge Cases:**
- **Invalid Names**: Explicitly blocks "." and ".." to prevent directory traversal or self-referencing errors.
- **No-Op Rename**: Handles the case where source and target paths are identical by succeeding without modifying the filesystem.
- **Non-Empty Target Directory**: Explicitly checks if the target directory is empty before allowing an overwrite, returning `Exist` if not, adhering to NFS semantics.

**Complexity:**
- **Time**: O(1) for path operations and comparisons. O(N) for checking if the target directory is empty (where N is the number of entries in the target directory). The actual `fs::rename` complexity depends on the underlying OS/filesystem.
- **Space**: O(1) for path buffers and metadata structures.

**Determinism:**
- **Deterministic**: The logic follows a strict sequence of checks and operations. The result depends entirely on the state of the filesystem and the input arguments.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::rename`**:
 - **`Rename` Trait**: The module implements this trait, specifically the `rename` method, ensuring that `MirrorFS` can act as a VFS backend for NFS rename requests.
 - **`Args`, `Success`, `Fail`**: These structures define the input and output contract. The implementation populates `WccData` in `Success` and `Fail` to satisfy the cache consistency requirements defined in the trait specification.

- **From `nfs_mamont::vfs`**:
 - **`Error` Enum**: The module uses specific variants like `InvalidArgument` and `Exist` to signal protocol-compliant error conditions back to the caller.
 - **`WccData`**: The module constructs these structures for both the source and target directories. It captures metadata *before* the operation and (in the success path or via helper methods) *after* the operation to populate the `before` and `after` fields.

- **From `mirrorfs::fs` (Assumptions based on usage)**:
 - **`path_for_handle`**: An internal method (not listed in public facts but used in code) assumed to resolve a `vfs::Handle` to a concrete `PathBuf`. This is critical for translating VFS handles to filesystem paths.
 - **`remove_cached_path` / `rename_cached_path`**: Internal async methods used to maintain the integrity of `MirrorFS`'s internal handle-to-path cache during the rename operation.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on entities defined in dependencies:
 - `rename::Args`
 - `rename::Success`
 - `rename::Fail`
 - `vfs::WccData`
 - `vfs::Error`

Relations:
- **Implementation**: `MirrorFS` implements `vfs::rename::Rename`.

Global Invariants:
- **WCC Reporting**: The function guarantees that `WccData` is returned for both the source and target directories in all code paths (success and failure), ensuring the client can update its cache even if the operation fails.
- **Atomicity**: The operation relies on `tokio::fs::rename`, which is atomic at the filesystem level for local filesystems. The implementation ensures that cache updates happen only after the filesystem operation succeeds.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped in `rename::Fail`.
 - `InvalidArgument`: Returned if names are "." or "..".
 - `Exist`: Returned if the target exists and is incompatible (type mismatch) or is a non-empty directory.
 - Other IO errors (mapped via `Self::io_error_to_vfs`): Returned if `fs::rename` or metadata retrieval fails.

Error Propagation Strategy:
- **Early Return with Context**: The function uses a pattern of checking conditions and returning `rename::Fail` immediately. Crucially, even on early returns (e.g., invalid names), it attempts to construct `WccData` (though it may be empty if path resolution failed) to satisfy the return type signature.

Recoverability:
- **Client-Side**: Errors like `InvalidArgument` or `Exist` indicate client request errors and are not recoverable by retrying the same request. IO errors might be transient.

Panics:
- **Allowed**: No explicit panics are present in the code. All error paths return `Result`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::rename::Rename`**: Implemented for `super::MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete implementation of the NFS RENAME procedure for the MirrorFS backend**. The system contains a layered architecture where the top layer handles NFS protocol specifics (RPC, arguments) and the bottom layer handles physical storage. This module sits in the middle, translating the abstract `Rename` request into specific filesystem operations (`tokio::fs::rename`) while managing the internal state (caching) of the `MirrorFS` driver.

The `MirrorFS` system requires this module because a simple filesystem rename is insufficient to satisfy the NFSv3 protocol. The protocol demands strict validation (e.g., forbidding "." and ".."), specific error handling for overwriting non-empty directories, and, most importantly, the reporting of Weak Cache Consistency (WCC) data for both the source and target directories. This module orchestrates these requirements: it captures metadata before the operation, performs the rename, updates internal path caches to reflect the new location, and captures metadata after the operation to construct the `WccData`.

A typical usage scenario involves an NFS client requesting to move a file. The `MirrorFS` instance receives the `Args` containing handles. This module resolves those handles to local paths, checks if the move is valid (e.g., target directory is empty), and executes the move. If successful, it updates the `MirrorFS` internal cache so that future lookups for the old path fail and lookups for the new path succeed. Finally, it returns the `WccData` to the client, allowing the client to invalidate its cache for the source directory and update it for the target directory.

Inside the system, the following things happen and they use this module:
1. **Protocol Enforcement**: The module enforces rules like checking for "." and ".." names, ensuring the backend does not perform illegal operations that could confuse the client or violate the protocol.
2. **Cache Coherence**: The module calls `remove_cached_path` and `rename_cached_path` on `MirrorFS`. This is critical because `MirrorFS` likely maintains a mapping of file handles to paths to optimize lookups; without updating this mapping during a rename, subsequent operations on the file would fail or reference stale data.
3. **WCC Data Collection**: The module meticulously gathers metadata before and after the operation. This is essential for the "Weak Cache Consistency" model of NFSv3, which allows clients to validate cached data without re-reading entire directories.

**Uncertainty**: The code references `self.path_for_handle`, `self.remove_cached_path`, and `self.rename_cached_path`. These methods are not listed in the public facts for `MirrorFS`. It is assumed they are internal (private or protected) async methods used for path resolution and cache management within the `MirrorFS` implementation. The logic of `Self::metadata`, `Self::io_error_to_vfs`, and `Self::wcc_data` is also assumed to be helper methods defined within the `MirrorFS` implementation or a parent module, as they are not imported from external crates.