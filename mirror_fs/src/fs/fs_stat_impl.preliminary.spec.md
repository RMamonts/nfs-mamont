<!-- SPEC_HASH: bc367b2a9746bb66e15432e586630078ad1a6ca8aa871803af2c846df9af328a -->
# Module Specification

Module: mirrorfs::fs::fs_stat_impl
Rust File: mirror_fs/src/fs/fs_stat_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::fs_stat`**: Used to import the `FsStat` trait, along with the `Args`, `Success`, and `Fail` types. This module provides the interface definition that `MirrorFS` must implement to support the NFSv3 `FSSTAT` procedure, allowing the server to respond to client requests for file system statistics.
- **`super::MirrorFS`**: The parent struct for which the `FsStat` trait is being implemented. The implementation relies on the internal state and methods of `MirrorFS` to resolve file handles to paths and retrieve file attributes.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `FsStat` for `MirrorFS`

**Intent:**
To provide a concrete implementation of the NFSv3 `FSSTAT` procedure for the `MirrorFS` backend. This allows the NFS server to report file system capacity and usage statistics (or lack thereof) when a client queries the `MirrorFS` export.

**Inputs:**
- `args: fs_stat::Args`: Contains the `root` file handle (`file::Handle`) identifying the file system instance to query.

**Outputs:**
- `Result<fs_stat::Success, fs_stat::Fail>`:
 - **`Success`**: Contains the attributes of the root handle and a set of statistics.
 - **`Fail`**: Contains a `vfs::Error` if the handle could not be resolved.

**Steps:**
1. **Handle Resolution**: The method calls `self.path_for_handle(&args.root).await` to translate the opaque NFS file handle into a concrete local filesystem path.
2. **Error Handling**: If `path_for_handle` returns an `Err`, the method immediately returns `fs_stat::Fail` containing the error and setting `root_attr` to `None`.
3. **Attribute Retrieval**: If the handle is resolved successfully, the method calls `Self::file_attr(&path)` to obtain the current attributes for the root path.
4. **Response Construction**: The method constructs a `fs_stat::Success` struct.
 - The `root_attr` field is populated with the result of `Self::file_attr`.
 - All statistics fields (`total_bytes`, `free_bytes`, `available_bytes`, `total_files`, `free_files`, `available_files`, `invarsec`) are hardcoded to `0`.
5. **Return**: The `Success` struct is wrapped in `Ok` and returned.

**Edge Cases:**
- **Invalid Handle**: If the provided `root` handle is stale or invalid, `path_for_handle` is expected to return an error, which is propagated to the client via `fs_stat::Fail`.
- **Attribute Retrieval Failure**: The code assumes `Self::file_attr` returns a type compatible with `Option<file::Attr>`. If `Self::file_attr` fails internally (e.g., due to I/O error), the behavior depends on that method's implementation (assumed to return `None` or handle the error internally).

**Complexity:**
- **Time**: O(1) or O(P) where P is the path depth, depending on the implementation of `path_for_handle` and `file_attr`.
- **Space**: O(1).

**Determinism:**
- **Deterministic**: The statistics returned are constant (zero), and the attributes depend deterministically on the state of the file at the resolved path.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::fs_stat`**:
 - **`FsStat` Trait**: This module implements the `fs_stat` method defined by this trait. The trait defines the contract for separating successful statistics retrieval from failure cases while preserving file attributes for cache consistency.
 - **`Args`**: This module uses the `root` field from `Args` as the input key for the operation.
 - **`Success` and `Fail`**: This module constructs these types to return the result to the VFS layer.

- **From `mirrorfs::fs::mod` (Assumed)**:
 - **`path_for_handle`**: Although not listed in the provided `facts.json`, the source code implies the existence of an asynchronous method `path_for_handle(&Handle) -> Result<PathBuf, vfs::Error>` on `MirrorFS`. This mechanism is critical for mapping the abstract NFS handle to a concrete location in the mirror.
 - **`file_attr`**: Although not listed in the provided `facts.json`, the source code implies the existence of a method `file_attr(&Path) -> Option<file::Attr>` (or a compatible type) on `MirrorFS`. This mechanism is used to populate the `root_attr` field in the response.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It utilizes `fs_stat::Args`, `fs_stat::Success`, and `fs_stat::Fail` from the `nfs_mamont::vfs::fs_stat` module.

Relations:
- **`MirrorFS` implements `FsStat`**: Realization relationship.

Global Invariants:
- **Zero Statistics**: The implementation enforces an invariant where all file system usage statistics (bytes, files, available space) are reported as zero. This suggests that `MirrorFS` either does not support quota reporting, is intended to appear as an empty/unlimited drive, or this is a placeholder implementation.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `fs_stat::Fail`. This error originates from the `path_for_handle` method if the handle cannot be resolved.

Error Propagation Strategy:
- **Early Return**: If `path_for_handle` fails, the error is immediately wrapped in `fs_stat::Fail` and returned. The `root_attr` in the `Fail` struct is explicitly set to `None`.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` returned by `path_for_handle` (e.g., `StaleFile` vs `IO`).

Panics:
- **Allowed**: No.
- **Uncertainty**: If `Self::file_attr` returns a `Result` instead of an `Option`, the code `root_attr: Self::file_attr(&path)` might cause a type mismatch or panic depending on how `Success` is constructed. However, based on the signature `root_attr: Option<file::Attr>` in `Success`, it is assumed `Self::file_attr` returns `Option<file::Attr>`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::fs_stat::FsStat`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **implement the NFSv3 `FSSTAT` procedure for the `MirrorFS` backend**, thereby enabling `MirrorFS` to satisfy the requirements of the `nfs_mamont::vfs::Vfs` super-trait. The system contains a complex architecture where storage backends are abstracted behind traits. This module is necessary because the generic `Vfs` interface requires a concrete implementation of `fs_stat` for any backend that wishes to be exported via the NFS server.

A typical usage scenario of the system involves a client issuing an `FSSTAT` request to check available space on an NFS export backed by `MirrorFS`. The RPC layer receives the request, extracts the file handle, and invokes the `fs_stat` method implemented in this module. The module attempts to resolve the handle to a local path. If successful, it returns a `Success` response containing the root directory's attributes. Notably, all capacity and usage fields (total/free bytes, files, etc.) are returned as zero.

Inside the system, the following things happen and they use this module:
1.  **Trait Satisfaction**: The `Vfs` trait (defined in `nfs_mamont::vfs::mod`) aggregates `FsStat`. By implementing `FsStat` here, `MirrorFS` can be passed to the server context as a valid `Vfs` implementation.
2.  **Handle Translation**: The module relies on `path_for_handle` to bridge the gap between the NFS protocol's opaque handles and the local filesystem paths managed by `MirrorFS`.
3.  **Stub Reporting**: The module acts as a stub for statistics reporting. By returning zeros, it effectively tells the client that no space information is available or that the space is unlimited/unknown, which might be acceptable for specific mirroring scenarios where the backend is read-only or where quota management is delegated to another layer.

**Uncertainty**: The provided `facts.json` for `mirrorfs::fs::mod` does not list the methods `path_for_handle` or `file_attr`. The analysis assumes these methods exist on `MirrorFS` with the signatures `async fn path_for_handle(&self, &Handle) -> Result<PathBuf, vfs::Error>` and `fn file_attr(&self, &Path) -> Option<file::Attr>` (or compatible) based on their usage in the source code.