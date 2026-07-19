<!-- SPEC_HASH: 425947699d7639c1529c41446a0fe384655929e3caeb89a8845d183610a20e75 -->
# Module Specification

Module: mirrorfs::fs::mk_dir_impl
Rust File: mirror_fs/src/fs/mk_dir_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs`**: Used to perform asynchronous filesystem operations, specifically `fs::create_dir`, to create the directory on the underlying storage.
- **`nfs_mamont::vfs`**: Used to import the `WccData` struct and the `Error` enum. These are required to construct the return types (`Success` and `Fail`) defined by the `MkDir` trait, ensuring compliance with the VFS interface.
- **`nfs_mamont::vfs::mk_dir`**: Used to import the `MkDir` trait and its associated types (`Args`, `Success`, `Fail`). This module provides the concrete implementation of this trait for the `MirrorFS` struct.
- **`super::MirrorFS`**: The struct for which the `MkDir` trait is implemented. It provides the context (root path) and helper methods (e.g., `path_for_handle`, `handle_for_path`) to translate between abstract VFS handles and concrete filesystem paths.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `MkDir` for `MirrorFS`

**Intent:**
To provide the concrete logic for creating a directory within the `MirrorFS` backend. This involves translating the abstract VFS request (handles and names) into a concrete filesystem path, performing the creation, applying initial attributes, and gathering the necessary metadata (handles and WCC data) to satisfy the NFSv3 protocol contract.

**Inputs:**
- `args: mk_dir::Args`: Contains the parent directory handle (`args.object.dir`), the name of the new directory (`args.object.name`), and the initial attributes (`args.attr`).

**Outputs:**
- `Result<mk_dir::Success, mk_dir::Fail>`:
 - `Success`: Contains the new directory's handle, its attributes, and the WCC data for the parent directory.
 - `Fail`: Contains the error that occurred and the WCC data for the parent directory.

**Steps:**
1. **Name Validation**: Calls `Self::ensure_name_allowed` with the directory name. If it returns an error, the function returns `Fail` immediately with empty WCC data.
2. **Path Resolution**: Calls `self.path_for_handle(&args.object.dir).await` to convert the parent directory's VFS handle into a filesystem path. If this fails, returns `Fail` with empty WCC data.
3. **Pre-Operation State Capture**: Calls `std::fs::symlink_metadata` on the parent directory path to capture its attributes *before* the modification. This is stored as `before`.
4. **Path Construction**: Creates the full path for the new directory by appending the new directory name to the parent path.
5. **Directory Creation**: Invokes `fs::create_dir(&child_path).await`. If this fails, the IO error is converted to a `vfs::Error` (via `Self::io_error_to_vfs`), and the function returns `Fail` containing the WCC data derived from the `before` state.
6. **Attribute Application**: Calls `Self::apply_set_attr(&child_path, &args.attr)` to set the initial attributes (mode, uid, gid, etc.) on the newly created directory. If this fails, returns `Fail` with the parent's WCC data.
7. **Post-Operation Metadata Retrieval**: Calls `Self::metadata(&child_path)` to get the attributes of the newly created directory and converts them to `file::Attr`. If this fails, returns `Fail` with the parent's WCC data.
8. **Handle Generation**: Calls `self.handle_for_path(&child_path).await` to generate a VFS handle for the new directory. If this fails, returns `Fail` with the parent's WCC data.
9. **WCC Calculation**: Calls `Self::wcc_data(&dir_path, before)` to construct the final `WccData` for the parent directory, which includes the pre-operation attributes captured in step 3 and the current post-operation attributes.
10. **Success Return**: Returns `Ok(Success)` populated with the new handle, attributes, and WCC data.

**Edge Cases:**
- **Invalid Names**: If the name validation fails, the operation aborts without attempting to access the filesystem.
- **Missing Parent**: If the parent handle cannot be resolved to a path, the operation fails.
- **Partial Creation**: If the directory is created but setting attributes fails, the directory remains on disk (standard filesystem behavior), but the operation returns an error to the client.

**Complexity:**
- **Time**: Dependent on the latency of the underlying filesystem operations (metadata lookups, directory creation).
- **Space**: O(1) for the control structures, plus the space required for the new directory entry on disk.

**Determinism:**
- **Non-deterministic**: The result depends on the state of the underlying filesystem (e.g., whether the directory already exists, permissions, disk space).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::mk_dir`**:
 - **`MkDir` Trait**: Defines the asynchronous interface `mk_dir` that this module implements. It dictates the input arguments (`Args`) and the structure of the return types (`Success`, `Fail`), enforcing the requirement to return WCC data and handles.
 - **`Args`, `Success`, `Fail`**: These structs provide the data containers used throughout the implementation. Specifically, `Success` requires the new `Handle` and `Attr`, while `Fail` requires the `Error` and `dir_wcc`.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: This module relies on the structure of `WccData` to package the pre- and post-operation attributes of the parent directory. The implementation captures `before` metadata explicitly to populate this structure correctly.
 - **`Error`**: Used to wrap any IO or internal errors encountered during the directory creation process, ensuring they are returned in a format compatible with the VFS layer.

- **From `mirrorfs::fs` (Assumed based on usage)**:
 - **`MirrorFS` Context**: The implementation relies on `MirrorFS` methods like `path_for_handle` and `handle_for_path` to bridge the gap between the VFS abstraction (handles) and the physical filesystem (paths). *Uncertainty: The specific signatures of `path_for_handle` and helper methods like `ensure_name_allowed` or `apply_set_attr` are not defined in the provided facts for `MirrorFS`, but their usage implies they handle the translation and validation logic.*

---

## 4. Data Model

Entities:
- This module defines no new public structs or enums. It operates entirely on the types defined by the `nfs_mamont::vfs::mk_dir` trait and the `MirrorFS` struct.

Relations:
- **Implementation**: `MirrorFS` implements the `mk_dir::MkDir` trait.
- **Translation**: The implementation logic maps `vfs::Handle` -> `PathBuf` (via `path_for_handle`) and `PathBuf` -> `vfs::Handle` (via `handle_for_path`).

Global Invariants:
- **WCC Consistency**: If the directory creation fails after the parent directory's metadata has been successfully read (`before`), the `Fail` result must contain valid WCC data reflecting the state of the parent directory before the failed attempt.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type returned within the `mk_dir::Fail` struct. This includes errors derived from IO operations (e.g., `IO`, `NoSpace`, `Access`) and validation errors (e.g., `Exist`).

Error Propagation Strategy:
- **Conversion and Wrapping**: IO errors from `tokio::fs` are converted to `vfs::Error` using `Self::io_error_to_vfs`. Errors from helper methods (e.g., `path_for_handle`, `handle_for_path`) are propagated as `vfs::Error`. All errors are wrapped in `mk_dir::Fail`, which also includes the `dir_wcc` data.

Recoverability:
- **Dependent on Error Code**:
 - `vfs::Error::Exist`: Indicates the directory already exists or the name is invalid. The client should not retry immediately.
 - `vfs::Error::IO`: May indicate a transient issue; the client might retry depending on the specific IO error.
 - `vfs::Error::Access`: Permission denied; retrying without changing permissions will fail.

Panics:
- **Allowed**: No. The implementation is designed to return `Result` types for all foreseeable error conditions.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::mk_dir::MkDir`**: Implemented for `mirrorfs::fs::MirrorFS`.

---

## 7. Overview

This module is used in order to **implement the NFSv3 `MKDIR` procedure for the `MirrorFS` backend**. The system requires this specific implementation because `MirrorFS` acts as a bridge between the generic NFS protocol (which operates on opaque handles and abstract attributes) and a concrete local filesystem (which operates on paths and standard file metadata). This module is necessary to translate the high-level request to create a directory into the specific sequence of low-level filesystem operations required to maintain consistency and satisfy the protocol contract.

A typical usage scenario of the system involves an NFS client sending a `MKDIR` request to create a folder named "logs" inside an existing directory. The request reaches the `MirrorFS` backend via the `MkDir` trait. This module resolves the parent directory's handle to a real path on the server's disk, creates the "logs" directory, applies any requested permissions or ownership, and then generates a new NFS handle for this directory. Crucially, it also captures the state of the parent directory before and after the operation to construct the `WccData`, which is sent back to the client to allow it to validate its cache.

Inside the system, the following things happen and they use this module:
1. **Path Translation**: The module uses `path_for_handle` to convert the abstract parent directory handle into a concrete filesystem path. This is essential because the underlying OS cannot understand NFS handles directly.
2. **Protocol Compliance**: By explicitly capturing the parent directory's metadata before creation (`std::fs::symlink_metadata`) and calculating WCC data even on failure, the module ensures that the NFSv3 Weak Cache Consistency requirements are met. This prevents the client from using stale cached data about the parent directory.
3. **Attribute Initialization**: The module integrates with `apply_set_attr` to ensure that the directory is created with the correct ownership and permissions immediately, rather than creating it with defaults and requiring a separate `SETATTR` call from the client.

The critical aspect of this module is the **orchestration of side effects**. It manages the transition from the abstract VFS world to the concrete IO world, ensuring that every step—validation, path resolution, creation, and metadata collection—is performed in the correct order and that errors are handled gracefully without leaving the VFS layer in an inconsistent state regarding the reported attributes.

**Uncertainty**: The source code references helper methods such as `Self::ensure_name_allowed`, `Self::apply_set_attr`, `Self::io_error_to_vfs`, and `Self::wcc_data`. These methods are not defined in the provided source snippet or the `MirrorFS` facts. Their behavior is inferred from their names and usage context (e.g., `ensure_name_allowed` likely checks for "." or "..", `apply_set_attr` likely applies `NewAttr` to a path). Additionally, `path_for_handle` is used but not listed in the `MirrorFS` public facts; it is assumed to be an internal or associated method capable of resolving a `Handle` to a `PathBuf`.