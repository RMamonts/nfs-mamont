<!-- SPEC_HASH: 1d1ea3f6f30983f6bccbe116820443c65f090c6c789fb3b851595c1bfce58d23 -->
# Module Specification

Module: mirrorfs::fs::commit_impl
Rust File: mirror_fs/src/fs/commit_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs::OpenOptions`**: Used to asynchronously open the target file on the local filesystem. It is configured with write permissions to allow the subsequent `sync_all` operation to flush data to stable storage.
- **`nfs_mamont::vfs::commit`**: Used to import the `Commit` trait, `Args`, `Success`, and `Fail` types. This module provides the concrete implementation of the `Commit` trait for the `MirrorFS` struct, defining how the NFSv3 COMMIT procedure is executed.
- **`nfs_mamont::vfs`**: Used to import the `WccData` and `Error` types. `WccData` is populated with file attributes before and after the operation to support Weak Cache Consistency, and `Error` is used to classify failures.
- **`std::fs`**: Used for `symlink_metadata` to synchronously retrieve file metadata (attributes) before the operation begins. This is necessary to capture the "before" state for the `WccData`.
- **`super::*`**: Used to access the `MirrorFS` struct definition and its associated helper methods (e.g., `path_for_handle`, `validate_regular`, `wcc_data`, `write_verifier`, `io_error_to_vfs`). These methods are assumed to be defined in the parent module `mirrorfs::fs` and handle the internal logic of path resolution, validation, and error mapping.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `nfs_mamont::vfs::commit::Commit` for `MirrorFS`

**Intent:**
To execute the NFSv3 COMMIT procedure for a file mirrored by `MirrorFS`. This involves ensuring that any previously written (unstable) data for the specified file is flushed to stable storage (disk) and returning the necessary cache consistency data and a write verifier to the client.

**Inputs:**
- `args: commit::Args`: Contains the `file` handle, `offset`, and `count`. Note: The implementation currently ignores `offset` and `count` and syncs the entire file, which is consistent with the behavior of `sync_all`.

**Outputs:**
- `Result<commit::Success, commit::Fail>`:
  - `Success`: Contains `file_wcc` (Weak Cache Consistency data) and `verifier` (to detect server reboots).
  - `Fail`: Contains `error` (the specific failure reason) and `file_wcc`.

**Steps:**
1. **Handle Resolution**: Calls `self.path_for_handle(&args.file).await` to translate the abstract NFS file handle into a concrete filesystem path. If this fails, returns `Fail` with empty `WccData` (since the path is unknown).
2. **Pre-Operation State Capture**: Calls `std::fs::symlink_metadata(&path)` to retrieve the file's current metadata. If successful, it converts this metadata into `WccAttr` (stored as `before`) and `Attr` (used for validation).
3. **Type Validation**: Calls `Self::validate_regular(&attr)` to ensure the target is a regular file. If validation fails (e.g., it is a directory), returns `Fail` with the specific error and the captured `WccData`.
4. **File Opening**: Uses `OpenOptions::new().write(true).open(&path).await` to open the file. This requires write access to perform the synchronization. If opening fails, the IO error is mapped to a VFS error via `Self::io_error_to_vfs`, and `Fail` is returned.
5. **Synchronization**: Calls `file.sync_all().await` to flush all file data and metadata associated with the file handle to the underlying storage device. If this fails, the error is mapped and `Fail` is returned.
6. **Success Response**: Calls `Self::wcc_data(&path, before)` to construct the final `WccData` (which likely fetches the "after" attributes internally) and `self.write_verifier()` to get the current state verifier. Returns `Success` with these values.

**Edge Cases:**
- **Invalid Handle**: If `path_for_handle` fails, the operation aborts immediately with empty `WccData`.
- **Metadata Access Failure**: If `symlink_metadata` fails (e.g., race condition where file is deleted), `before_meta` is `None`. The operation may proceed or fail depending on subsequent checks, but `WccData` will lack the "before" state.
- **Non-Regular File**: Attempting to commit a directory or special device results in a validation error.

**Complexity:**
- Time: O(1) for metadata and handle resolution + O(N) for `sync_all`, where N is the amount of data in the OS cache that needs to be written to disk.
- Space: O(1).

**Determinism:**
- Non-deterministic. The result depends on the state of the underlying filesystem and the success of I/O operations.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::commit`**:
 - **`Commit` Trait**: The module implements this trait, specifically the `commit` method. This ensures that `MirrorFS` adheres to the NFSv3 contract for committing unstable writes.
 - **`Args`, `Success`, `Fail`**: These structures define the input and output contract. The implementation populates `Fail` with `error` and `file_wcc`, and `Success` with `file_wcc` and `verifier`.

- **From `nfs_mamont::vfs::file`**:
 - **`Attr` and `WccAttr`**: These types are used to store the file metadata extracted via `symlink_metadata`. They are essential for constructing the `WccData` required by the NFS protocol.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: The structure wrapping the before/after attributes. The module relies on helper methods (assumed in `super`) to populate this structure.
 - **`Error`**: The error type used within `Fail` to signal specific NFS error conditions (e.g., `IO`, `NotDir`).

- **From `mirrorfs::fs` (Assumed based on usage)**:
 - **`path_for_handle`**: Critical for mapping the abstract NFS handle to a local path.
 - **`validate_regular`**: Critical for ensuring the operation is only performed on valid file types.
 - **`write_verifier`**: Critical for generating the cookie used to detect server reboots.
 - **`io_error_to_vfs`**: Critical for translating standard Rust IO errors into NFS-specific error codes.

---

## 4. Data Model

Entities:
- This module defines no new structs or enums. It implements a trait for the `MirrorFS` struct defined in the parent module.

Relations:
- **Implementation**: `MirrorFS` implements `nfs_mamont::vfs::commit::Commit`.

Global Invariants:
- **Write Requirement**: The implementation attempts to open the file with write permissions (`write(true)`) to perform the sync. This implies the user/process running the server must have write access to the file, even if no data modification is intended.

## 5. Error Model

Error Types:
- **`commit::Fail`**: The error wrapper returned by the function. It contains a `vfs::Error` and `WccData`.

Error Propagation Strategy:
- **Early Return**: The function uses an early return strategy. If any step fails (handle resolution, validation, open, sync), it immediately constructs and returns a `Fail` struct containing the error and the `WccData` captured up to that point.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` returned. For example, `IO` errors might be transient (disk full, network glitch), while `Stale` or `BadFileHandle` errors require the client to perform a new lookup.

Panics:
- **Allowed**: Indirectly.
- **Conditions**:
 - If the helper methods `path_for_handle`, `validate_regular`, or `wcc_data` (defined in `super`) panic.
 - If the Tokio runtime panics during the `await` calls.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::commit::Commit`**: The asynchronous trait defining the commit operation.

---

## 7. Overview

This module is used in order to **implement the NFSv3 COMMIT procedure** for the `MirrorFS` backend. The system contains a complex NFS server where clients may perform "unstable" writes (data acknowledged by the server but not yet written to disk). The `Commit` procedure is the mechanism by which a client explicitly requests that this data be flushed to stable storage. Furthermore, because unstable writes reside in volatile memory, a server reboot could lose this data. The `Commit` response includes a `verifier` that allows the client to detect if such a reboot occurred.

A typical usage scenario of the system involves a client sending a COMMIT request after a series of WRITE operations. The RPC layer parses the request into `Args` and calls `MirrorFS::commit`. This implementation resolves the file handle to a local path, captures the current file attributes for cache consistency, validates that the target is a regular file, and then opens the file. It calls `sync_all` to ensure all data is persisted to the physical disk. Finally, it returns the updated attributes and a write verifier to the client.

Inside the system, the following things happen and they use this module:
1.  **Data Durability**: The `sync_all` call ensures that the OS buffers are flushed to the storage device, guaranteeing that the data survives a power loss or server crash.
2.  **Cache Consistency**: By capturing metadata before the operation and returning `WccData` (which includes post-operation metadata), the module allows the client to synchronize its attribute cache with the server's state.
3.  **State Verification**: The `write_verifier` returned in `Success` allows the client to compare it with the verifier from a previous `WRITE`. A mismatch indicates a server reboot, signaling the client that unstable data may have been lost.

**Uncertainty**: The implementation details of helper methods such as `path_for_handle`, `validate_regular`, `wcc_data`, and `write_verifier` are not present in the provided source snippet. They are assumed to be defined in the parent module `mirrorfs::fs` (imported via `super::*`). The logic for how `wcc_data` fetches the "after" attributes is inferred from its usage but not explicitly visible here.