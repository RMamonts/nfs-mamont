<!-- SPEC_HASH: 425947699d7639c1529c41446a0fe384655929e3caeb89a8845d183610a20e75 -->
# Module Specification

Module: mirrorfs::fs::mk_dir_impl
Rust File: mirror_fs/src/fs/mk_dir_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs`**: Used to perform asynchronous file system operations, specifically `fs::create_dir`, which creates the actual directory on the underlying storage.
- **`nfs_mamont::vfs`**: Used to import the `vfs` module path and the `mk_dir` module. It provides the core types like `WccData` and `Error` that are required to satisfy the VFS contract.
- **`nfs_mamont::vfs::mk_dir`**: Used to import the `MkDir` trait and its associated types (`Args`, `Success`, `Fail`). This module provides the implementation of this trait for `MirrorFS`.
- **`super::MirrorFS`**: The struct for which the `MkDir` trait is being implemented. The implementation relies on several internal helper methods of `MirrorFS` (or its parent module) such as `path_for_handle`, `handle_for_path`, `ensure_name_allowed`, `apply_set_attr`, and various metadata conversion utilities.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `vfs::mk_dir::MkDir` for `MirrorFS`

**Intent:**
To create a directory on the local file system represented by `MirrorFS` while adhering to the NFSv3 protocol requirements. This involves validating the requested name, creating the directory, applying initial attributes, and returning the necessary handle and Weak Cache Consistency (WCC) data.

**Inputs:**
- `args: mk_dir::Args`: Contains the parent directory handle (`args.object.dir`), the name of the new directory (`args.object.name`), and the initial attributes (`args.attr`).

**Outputs:**
- `Result<mk_dir::Success, mk_dir::Fail>`:
 - `Success`: Contains the new directory's handle, its attributes, and the WCC data for the parent directory.
 - `Fail`: Contains a `vfs::Error` and the WCC data for the parent directory.

**Steps:**
1. **Name Validation**: Calls `Self::ensure_name_allowed` with the new directory name. If it returns an error, the function immediately returns `Fail` with empty WCC data.
2. **Path Resolution**: Calls `self.path_for_handle(&args.object.dir).await` to convert the parent directory's NFS handle into a file system path. If this fails, returns `Fail` with empty WCC data.
3. **Pre-Operation Metadata Capture**: Calls `std::fs::symlink_metadata` on the parent directory path to capture its state *before* the modification. This is stored as `before` metadata.
4. **Directory Creation**: Constructs the full path for the new child directory and calls `fs::create_dir(&child_path).await`.
 - If this fails, the IO error is converted to `vfs::Error` via `Self::io_error_to_vfs`, and `Fail` is returned with WCC data constructed from the `before` metadata and the current state of the parent directory.
5. **Attribute Application**: Calls `Self::apply_set_attr(&child_path, &args.attr)` to set the initial attributes (mode, uid, gid, etc.) specified in the arguments.
 - If this fails, `Fail` is returned with WCC data.
6. **Post-Operation Metadata Retrieval**: Calls `Self::metadata(&child_path)` to get the attributes of the newly created directory. These are converted to `file::Attr` via `Self::attr_from_metadata`.
 - If this fails, `Fail` is returned with WCC data.
7. **Handle Generation**: Calls `self.handle_for_path(&child_path).await` to generate an NFS handle for the new directory.
 - If this fails, `Fail` is returned with WCC data.
8. **Success Response**: Returns `Success` containing the new handle, the new attributes, and the WCC data for the parent directory (calculated via `Self::wcc_data` using the `before` snapshot and the current parent state).

**Edge Cases:**
- **Invalid Names**: If the name is invalid (e.g., "." or ".."), the operation fails early without modifying the file system.
- **Parent Directory Inaccessibility**: If the parent handle cannot be resolved to a path, the operation fails.
- **Race Conditions**: The implementation captures `before` metadata before attempting creation. If creation fails, it attempts to capture `after` metadata for the WCC response, ensuring the client receives the current state of the parent directory even if the operation failed.

**Complexity:**
- **Time**: O(1) relative to logical operations, though bounded by the latency of the underlying file system syscalls (metadata lookups, directory creation).
- **Space**: O(1) for path manipulation and metadata structures.

**Determinism:**
- **Non-deterministic**: The result depends on the state of the underlying file system (e.g., whether the directory already exists, permissions) and the success of system calls.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::mk_dir`**:
 - **`MkDir` Trait**: This module implements the `mk_dir` method defined by this trait. It strictly follows the contract regarding input arguments (`Args`) and return types (`Result<Success, Fail>`).
 - **`Args`, `Success`, `Fail`**: These structs define the data flow. The implementation extracts `object.dir` and `object.name` from `Args` and populates `Success` or `Fail` as required.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: The implementation constructs this structure (via `Self::wcc_data`) to satisfy the NFSv3 requirement of reporting parent directory changes. It captures metadata before the operation and attempts to capture it after (even on failure) to populate this structure.
 - **`Error`**: Used to wrap any IO or logic errors occurring during the process (e.g., `IO`, `Exist`, `Access`).

- **From `mirrorfs::fs` (Assumed based on usage)**:
 - **`MirrorFS` Internal Helpers**: The implementation relies on several methods not visible in the public facts but used in the code:
 - `path_for_handle`: Resolves an abstract handle to a concrete `PathBuf`.
 - `handle_for_path`: Generates a handle from a concrete `PathBuf`.
 - `ensure_name_allowed`: Validates the directory name.
 - `apply_set_attr`: Applies attributes to a path.
 - `io_error_to_vfs`: Converts `std::io::Error` to `vfs::Error`.
 - `wcc_attr_from_metadata` / `attr_from_metadata`: Convert raw OS metadata to VFS attribute types.
 - `wcc_data`: Constructs the final `WccData` struct.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates entirely on types defined in `nfs_mamont::vfs::mk_dir` and `nfs_mamont::vfs`.

Relations:
- **Implementation Relation**: `MirrorFS` implements `mk_dir::MkDir`.

Global Invariants:
- **WCC Consistency**: If the operation fails after the parent directory path is successfully resolved, the `Fail` response must contain valid `dir_wcc` data (reflecting the state of the parent directory after the attempt), rather than empty WCC data.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type returned within the `mk_dir::Fail` struct. This includes errors like `Exist` (if the directory already exists or name is invalid), `IO` (for disk failures), `Access` (for permission issues), etc.

Error Propagation Strategy:
- **Conversion and Wrapping**: Errors from `tokio::fs` (std::io::Error) are converted to `vfs::Error` using `Self::io_error_to_vfs`. Errors from internal helpers (like `path_for_handle`) are typically already `vfs::Error` or converted. All errors are wrapped in `mk_dir::Fail`.
- **WCC Preservation**: The implementation ensures that even when returning an error, it attempts to provide `dir_wcc` data if the parent directory path was successfully resolved, allowing the client to update its cache for the parent directory despite the failure.

Recoverability:
- **Dependent on Error Code**:
 - `vfs::Error::Exist`: Indicates the directory already exists or the name is reserved. The client should not retry immediately without changing the name.
 - `vfs::Error::IO`: Transient disk error. The client might retry.
 - `vfs::Error::Access`: Permission denied. Retrying will fail without permission changes.

Panics:
- **Allowed**: No explicit panics are present in the code. The implementation uses `?` and `match` to handle errors gracefully.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::mk_dir::MkDir`**: Implemented for `mirrorfs::fs::MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete logic for creating directories within the `MirrorFS` backend**, bridging the high-level NFS protocol requirements with low-level file system operations. The system requires this module because the generic `nfs_mamont` framework defines *what* a directory creation operation looks like (inputs, outputs, error handling), but it relies on specific implementations like `MirrorFS` to define *how* it happens on the actual storage medium.

The `MirrorFS` system acts as a local filesystem adapter for the NFS server. It translates abstract NFS handles into file paths and executes standard OS calls. This module is critical because the `MKDIR` operation in NFSv3 is not atomic in the same way a simple `mkdir` syscall is; it requires the server to perform several steps (validation, creation, attribute setting) and return a complex result set that includes the new directory's handle, its full attributes, and the Weak Cache Consistency (WCC) data for the parent directory.

A typical usage scenario involves an NFS client requesting the creation of a directory named "logs" inside an existing directory handle. The `MirrorFS` implementation receives this request, validates that "logs" is a legal name, resolves the parent handle to a path (e.g., `/mnt/exports/data`), and calls `tokio::fs::create_dir` to create `/mnt/exports/data/logs`. It then applies any requested permissions or ownership changes. Finally, it generates a new NFS handle for "logs", reads its attributes, and snapshots the parent directory's modification time to construct the WCC data. This comprehensive response is sent back to the client, allowing it to update its local cache and immediately use the new directory handle.

Inside the system, the following things happen and they use this module:
1. **Protocol Enforcement**: The module ensures that even if the underlying OS call fails, the response structure matches the NFSv3 specification. For example, if `create_dir` fails because the directory already exists, the module maps this to `vfs::Error::Exist` and ensures the `dir_wcc` field is populated so the client knows the parent directory's state hasn't changed (or has, depending on the specific error).
2. **Metadata Synchronization**: By explicitly calling `apply_set_attr` after creation, the module ensures that the directory's attributes (mode, uid, gid) match the client's request atomically from the client's perspective, even if it requires multiple syscalls.
3. **Handle Management**: The module integrates with `MirrorFS`'s handle generation logic (`handle_for_path`) to ensure that the newly created directory is immediately addressable via the NFS protocol.

**Uncertainty**: The source code references several helper methods (`ensure_name_allowed`, `path_for_handle`, `apply_set_attr`, `io_error_to_vfs`, `wcc_attr_from_metadata`, `wcc_data`, `metadata`, `attr_from_metadata`) which are not defined in the provided public interface of `MirrorFS`. The specification assumes these are internal methods of `MirrorFS` or the `super` module that handle the specifics of path manipulation, attribute mapping, and error translation.