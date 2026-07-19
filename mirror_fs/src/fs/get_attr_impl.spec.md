<!-- SPEC_HASH: a4b0a3751a3f05421c882b4a3b033a719756dd089b492c7f044dca347fd8b41e -->
# Module Specification

Module: mirrorfs::fs::get_attr_impl
Rust File: src/fs/get_attr_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::get_attr`**: Used to import the `GetAttr` trait, along with its associated types `Args`, `Success`, and `Fail`. This module provides the contract that the `MirrorFS` struct must implement to support the NFSv3 `GETATTR` procedure.
- **`super::MirrorFS`**: The parent struct for which the trait is being implemented. The implementation relies on internal methods of `MirrorFS` (specifically `path_for_handle`, `metadata`, and `attr_from_metadata`) to perform the actual filesystem lookups and data translation.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `GetAttr` for `MirrorFS`

**Intent:**
To bridge the abstract NFS `GETATTR` operation, which uses opaque file handles, with the concrete local filesystem operations, which use file paths and system metadata. This mechanism allows the `MirrorFS` backend to serve file attributes to the NFS server.

**Inputs:**
- `args: get_attr::Args`: Contains the `file::Handle` (an opaque identifier) representing the target file system object.

**Outputs:**
- `Result<get_attr::Success, get_attr::Fail>`:
  - `Ok(Success)`: Contains `file::Attr`, representing the object's metadata (type, size, permissions, timestamps).
  - `Err(Fail)`: Contains `vfs::Error`, indicating failure (e.g., stale handle, I/O error).

**Steps:**
1. **Handle Resolution**: The method calls `self.path_for_handle(&args.file).await`. This translates the NFS `file::Handle` into a concrete local filesystem path (`PathBuf`).
2. **Error Handling (Resolution)**: If `path_for_handle` returns an `Err`, the method immediately returns `get_attr::Fail` wrapping that error.
3. **Metadata Retrieval**: The method calls `Self::metadata(&path)`. This performs a synchronous system call (e.g., `stat`) to retrieve the low-level metadata for the file at the resolved path.
4. **Attribute Translation**: The method calls `Self::attr_from_metadata(&meta)`. This converts the raw system metadata into the standardized `file::Attr` structure required by the NFS protocol.
5. **Result Construction**: If successful, it returns `get_attr::Success` containing the attributes. If `metadata` fails, it returns `get_attr::Fail` wrapping the error.

**Edge Cases:**
- **Race Condition**: The file might be deleted or moved between the `path_for_handle` call and the `metadata` call. The `metadata` call is expected to return an appropriate `vfs::Error` (e.g., `NoEntry`) in such cases.
- **Invalid Handle**: If the provided `file::Handle` does not correspond to any known path, `path_for_handle` fails, and the operation aborts.

**Complexity:**
- **Time**: O(1) for handle lookup (assuming a hash map or similar index) + O(1) for the system `stat` call.
- **Space**: O(1) for the path and metadata structures (stack allocated).

**Determinism:**
- **Deterministic**: Given a specific filesystem state and handle, the output is deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::get_attr`**:
  - **`GetAttr` Trait**: Defines the asynchronous interface `async fn get_attr`. This module provides the concrete implementation for `MirrorFS`, satisfying the VFS contract.
  - **`Args`, `Success`, `Fail`**: These types structure the input and output. The implementation extracts the handle from `Args` and constructs either `Success` (with attributes) or `Fail` (with an error).

- **From `mirrorfs::fs` (Assumed Internal Mechanics)**:
  - **`path_for_handle`**: Although not listed in the provided public JSON for `MirrorFS`, the code implies this method exists. It is critical for mapping the protocol-level handle to a filesystem path.
  - **`metadata`**: Implied method used to access system-specific file information.
  - **`attr_from_metadata`**: Implied method used to normalize system information into the NFS `Attr` format.

---

## 4. Data Model

Entities:
- This module defines no new structs or enums. It operates entirely on types defined in `nfs_mamont::vfs::get_attr` and `nfs_mamont::vfs::file`.

Relations:
- **`MirrorFS` implements `GetAttr`**: This module establishes the implementation relationship, allowing `MirrorFS` to act as a VFS backend.

Global Invariants:
- **Handle Validity**: The implementation assumes that if `path_for_handle` returns `Ok(path)`, the path is valid for the current filesystem context.
- **Error Consistency**: Errors returned by `path_for_handle` and `metadata` must be compatible with `vfs::Error`, as they are directly wrapped in `get_attr::Fail`.

---

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type returned within `get_attr::Fail`. This encompasses standard NFS errors (e.g., `IO`, `StaleFile`, `NoEntry`).

Error Propagation Strategy:
- **Direct Wrapping**: Errors from internal methods (`path_for_handle`, `metadata`) are not transformed or filtered; they are wrapped directly into `get_attr::Fail { error }`.

Recoverability:
- **Dependent on Error Source**:
  - If `path_for_handle` returns `StaleFile`, the client needs to perform a new lookup.
  - If `metadata` returns `IO`, the operation might be retryable if the condition is transient.

Panics:
- **Allowed**: No. The implementation uses `match` and `return Err` to handle all potential error paths from the internal methods.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::get_attr::GetAttr`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **enable the `MirrorFS` backend to respond to NFSv3 `GETATTR` requests**. The system contains a complex architecture where the `nfs_mamont` server handles network protocols and RPC dispatching, while `MirrorFS` handles the actual storage logic on a local filesystem. This module is necessary because it connects the generic VFS interface (defined in `nfs_mamont`) to the specific implementation details of `MirrorFS`.

A typical usage scenario of the system involves an NFS client requesting the attributes of a file (e.g., to check size or modification time). The `nfs_mamont` server receives the request, extracts the file handle, and calls the `get_attr` method on the `MirrorFS` instance. This implementation then translates that handle into a local path, queries the operating system for the file's metadata, converts that metadata into the NFS format, and returns it to the server.

Inside the system, the following things happen and they use this module:
1.  **Protocol Translation**: The module acts as the adapter that converts the abstract "File Handle" concept used by the NFS protocol into the concrete "Path" concept used by the OS filesystem.
2.  **Data Normalization**: It ensures that the raw metadata retrieved from the OS (which might vary by platform) is converted into the standardized `file::Attr` structure expected by the NFS client.
3.  **Error Mapping**: It ensures that filesystem-level errors (like "file not found" or "permission denied") are mapped to the standard `vfs::Error` codes that the NFS server understands.

**Uncertainty**: The provided public interfaces JSON for `mirrorfs::fs` does not list the methods `path_for_handle`, `metadata`, or `attr_from_metadata`. However, the source code in this module explicitly calls `self.path_for_handle(...).await`, `Self::metadata(...)`, and `Self::attr_from_metadata(...)`. Therefore, it is assumed that these methods exist within the `MirrorFS` struct, likely as private or `pub(crate)` methods, or that the provided JSON for the dependency is incomplete. The specification assumes these methods perform the logical operations implied by their names (handle-to-path resolution, stat call, and metadata conversion).