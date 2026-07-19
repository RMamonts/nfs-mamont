<!-- SPEC_HASH: 1d1ea3f6f30983f6bccbe116820443c65f090c6c789fb3b851595c1bfce58d23 -->
# Module Specification

Module: mirrorfs::fs::commit_impl
Rust File: mirror_fs/src/fs/commit_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs::OpenOptions`**: Used to asynchronously open the file identified by the resolved path. The file is opened in write mode to ensure a valid file descriptor exists for the synchronization operation.
- **`std::fs`**: Used to perform a blocking `symlink_metadata` call. This captures the file system attributes (metadata) *before* the commit operation to populate the Weak Cache Consistency (WCC) `before` data.
- **`nfs_mamont::vfs::commit`**: Used to import the `Commit` trait, `Args`, `Success`, and `Fail` types. This module provides the implementation of the `Commit` trait for the `MirrorFS` struct, adhering to the NFSv3 protocol contract.
- **`super::*`**: Used to access the `MirrorFS` struct context and its associated helper methods (e.g., `path_for_handle`, `write_verifier`, `wcc_data`) which are assumed to be defined in the parent module `mirrorfs::fs`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `nfs_mamont::vfs::commit::Commit` for `MirrorFS`

**Intent:**
To execute the NFSv3 `COMMIT` procedure on the local file system managed by `MirrorFS`. This involves resolving the abstract file handle to a concrete path, capturing pre-operation metadata, validating the file type, and forcing cached data to stable storage.

**Inputs:**
- `&self`: A reference to the `MirrorFS` instance.
- `args: commit::Args`: Arguments containing the `file` handle, `offset`, and `count`. Note: The current implementation ignores `offset` and `count` and commits the entire file.

**Outputs:**
- `Result<commit::Success, commit::Fail>`:
 - `Success`: Contains `file_wcc` (Weak Cache Consistency data) and a `verifier`.
 - `Fail`: Contains the `error` details and `file_wcc`.

**Steps:**
1. **Path Resolution**: Calls `self.path_for_handle(&args.file).await` to translate the NFS file handle into a local file system path. If this fails, returns `Fail` with empty WCC data.
2. **Pre-Operation Metadata Snapshot**: Calls `std::fs::symlink_metadata(&path)` to retrieve the file's metadata before modification. This is converted to `WccAttr` via `Self::wcc_attr_from_metadata`.
3. **Type Validation**: Converts the metadata to a full `Attr` struct and calls `Self::validate_regular`. If the file is not a regular file (e.g., a directory), returns `Fail`.
4. **File Opening**: Uses `OpenOptions::new().write(true).open(&path).await` to obtain a file handle. If opening fails (e.g., permission denied), returns `Fail` with the pre-operation WCC data.
5. **Synchronization**: Calls `file.sync_all().await` to flush all file data and metadata to the underlying storage device. If this fails, returns `Fail` with the pre-operation WCC data.
6. **Response Construction**: Calls `Self::wcc_data(&path, before)` to construct the final `WccData` (which presumably fetches post-operation metadata internally) and `self.write_verifier()` to get the current write verifier. Returns `Success`.

**Edge Cases:**
- **Invalid Handle**: If `path_for_handle` fails, the operation aborts immediately.
- **Non-Regular File**: If the target is a directory or special file, the operation fails.
- **Ignored Range**: The `offset` and `count` fields in `args` are effectively ignored; `sync_all` ensures the *entire* file is durable, not just the requested range.

**Complexity:**
- **Time**: Dominated by I/O latency (metadata lookup, file open, disk sync). Non-deterministic.
- **Space**: O(1) additional memory (excluding kernel buffers).

**Determinism:**
- **Non-deterministic**: Depends on the state of the local file system and hardware latency.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::commit`**:
 - **`Args`**: Defines the input structure. The implementation specifically relies on the `file` field (Handle) to identify the target.
 - **`Success` and `Fail`**: Defines the output structure. The implementation ensures that `file_wcc` is populated in both success and failure cases, and `verifier` is populated on success.
 - **`Commit` Trait**: Defines the asynchronous interface `commit` that this module implements.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: Used as the input key to resolve the file path.
 - **`Attr` and `WccAttr`**: Used as intermediate data structures to convert raw OS metadata into the VFS domain model.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: The structure returned in the result, encapsulating the state of the file before and after the commit attempt.

---

## 4. Data Model

Entities:
- This module defines no new public structs or enums. It operates entirely on types defined in `nfs_mamont::vfs` and `nfs_mamont::vfs::commit`.

Relations:
- **`MirrorFS` implements `Commit`**: The module provides the concrete logic for the abstract `Commit` trait defined in the dependency.

Global Invariants:
- **Full File Sync**: Regardless of the `offset` and `count` provided in the arguments, the implementation guarantees that `sync_all` is called on the file, persisting the entire file content and metadata.

## 5. Error Model

Error Types:
- **`commit::Fail`**: The wrapper error type returned to the caller. It contains a `vfs::Error` and `WccData`.

Error Propagation Strategy:
- **Early Return**: The function uses a series of `match` and `if let Err` blocks. If any step (path resolution, validation, open, sync) fails, it immediately constructs a `commit::Fail` struct and returns it.
- **Context Preservation**: In error cases, the `file_wcc` field is populated using `Self::wcc_data(&path, before)`, ensuring the client receives the pre-operation attributes even if the commit failed.

Recoverability:
- **Dependent on `vfs::Error`**: The specific error code inside `Fail` determines recoverability (e.g., `IO` might be transient, `Stale` requires a new lookup).

Panics:
- **Allowed**: No explicit panics are present in the code.
- **Conditions**: Potential panics could arise from the internal helper methods (`Self::wcc_data`, `Self::write_verifier`) if their invariants are violated, but these are not visible in this snippet.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::commit::Commit`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete implementation of the NFSv3 COMMIT procedure for the MirrorFS backend**. The system contains a complex architecture where the `nfs_mamont` crate defines the generic interfaces for NFS operations, and `mirror_fs` provides a specific implementation that mirrors data to a local file system. This module is necessary because it bridges the abstract VFS request (defined by `nfs_mamont::vfs::commit`) to concrete file system operations (opening and syncing files using Tokio and Std FS).

A typical usage scenario of the system involves an NFS client requesting a commit of unstable writes. The RPC layer receives the request, extracts the file handle and range, and calls the `commit` method on the `MirrorFS` instance. This module resolves the handle to a local path, ensures the file is a regular file, and then forces the operating system to flush all cached data for that file to the disk. It returns the updated attributes and a write verifier to the client, confirming data durability.

Inside the system, the following things happen and they use this module:
1. **Handle Translation**: The module relies on `path_for_handle` (assumed to be in `super`) to map the opaque NFS file handle to a concrete file system path.
2. **State Verification**: By calling `write_verifier`, the module provides the client with a mechanism to detect server reboots, as required by the NFS specification.
3. **Cache Consistency**: The module captures metadata before and after the sync operation (via `symlink_metadata` and `wcc_data`) to populate the `WccData` structure, allowing the client to synchronize its attribute cache.

**Uncertainty**:
- The methods `path_for_handle`, `wcc_attr_from_metadata`, `attr_from_metadata`, `validate_regular`, `io_error_to_vfs`, `wcc_data`, and `write_verifier` are called on `Self` but are not defined in the provided code snippet or the public facts of `MirrorFS`. It is assumed that these are internal helper methods defined in the parent module `mirrorfs::fs` or a private trait implemented by `MirrorFS`.
- The implementation of `wcc_data` is not visible, but based on its usage (`Self::wcc_data(&path, before)`), it is assumed to fetch the post-operation metadata internally to construct the full `WccData` struct.