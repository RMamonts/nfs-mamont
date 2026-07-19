<!-- SPEC_HASH: a4b0a3751a3f05421c882b4a3b033a719756dd089b492c7f044dca347fd8b41e -->
# Module Specification

Module: mirrorfs::fs::get_attr_impl
Rust File: src/fs/get_attr_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::get_attr`**: Used to import the `GetAttr` trait and its associated types (`Args`, `Success`, `Fail`). This module provides the interface definition that `MirrorFS` must implement to support the NFSv3 `GETATTR` procedure.
- **`super::MirrorFS`**: The parent struct for which this implementation block provides logic. This struct is assumed to contain the internal state and helper methods required to map NFS file handles to local filesystem paths and retrieve system metadata.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `GetAttr` for `MirrorFS`

**Intent:**
To bridge the abstract NFS protocol request for file attributes with the concrete local filesystem operations. This mechanism translates an opaque file handle into a filesystem path, queries the operating system for metadata, and then normalizes that metadata into the NFS attribute format.

**Inputs:**
- `args: get_attr::Args`: Contains the `file::Handle` which is the opaque identifier for the file system object.

**Outputs:**
- `Result<get_attr::Success, get_attr::Fail>`:
  - `Ok(Success)`: Contains the `file::Attr` structure populated with the object's metadata.
  - `Err(Fail)`: Contains a `vfs::Error` indicating failure (e.g., stale handle, I/O error).

**Steps:**
1. **Handle Resolution**: The method calls `self.path_for_handle(&args.file).await`. This asynchronous operation attempts to map the provided NFS file handle to a concrete path on the local filesystem.
2. **Error Handling (Resolution)**: If `path_for_handle` returns an `Err`, the method immediately returns `get_attr::Fail` wrapping that error. This typically handles cases where the handle is invalid or stale.
3. **Metadata Retrieval**: If the path is successfully resolved, the method calls `Self::metadata(&path)`. This is assumed to be a helper function that performs a system call (e.g., `stat`) to retrieve the file's metadata.
4. **Error Handling (Retrieval)**: If `metadata` returns an `Err` (e.g., file not found, permission denied), the method returns `get_attr::Fail` wrapping that error.
5. **Attribute Conversion**: If metadata is successfully retrieved, the method calls `Self::attr_from_metadata(&meta)`. This helper converts the OS-specific metadata structure into the standardized `file::Attr` structure used by the NFS protocol.
6. **Success**: The method returns `get_attr::Success` containing the converted attributes.

**Edge Cases:**
- **Stale Handle**: If the file handle cannot be resolved to a path, the error is propagated immediately.
- **Race Condition**: The file might be deleted or moved between the time the handle is resolved to a path and the time `metadata` is called. The `metadata` call is expected to handle this (e.g., returning `NoEntry`).

**Complexity:**
- **Time**: O(1) or O(L) where L is the path length, depending on the implementation of `path_for_handle` and the underlying filesystem syscall.
- **Space**: O(1) for the control flow, plus the size of the `file::Attr` struct.

**Determinism:**
- **Deterministic**: Given a specific filesystem state and a valid handle, the result is deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::get_attr`**:
  - **`GetAttr` Trait**: The module implements this trait, specifically the `async fn get_attr` method. This ensures that `MirrorFS` conforms to the contract expected by the NFS server's RPC layer.
  - **`Args`, `Success`, `Fail`**: The module uses these types to strictly type the input and output of the operation, ensuring that the data exchanged matches the NFSv3 specification.

- **From `mirrorfs::fs` (Assumed)**:
  - **`path_for_handle`**: Although not listed in the public facts JSON, the code relies on this method to perform the reverse lookup of `Handle` -> `Path`. This is critical for translating the network protocol identifier to a local resource.
  - **`metadata`**: The code relies on this associated function to perform the actual filesystem inspection.
  - **`attr_from_metadata`**: The code relies on this associated function to convert raw OS metadata into the `file::Attr` format.

---

## 4. Data Model

Entities:
- This module defines no new structs or enums. It operates entirely on types defined in `nfs_mamont::vfs::get_attr` and `nfs_mamont::vfs::file`.

Relations:
- **`MirrorFS` implements `GetAttr`**: This is the sole relation defined in this file, linking the concrete filesystem implementation to the generic VFS interface.

Global Invariants:
- **Handle Validity**: The implementation assumes that if `path_for_handle` returns `Ok(path)`, that path points to a valid filesystem object (though it might have been deleted immediately after).
- **Error Mapping**: Errors returned by `path_for_handle` and `metadata` must be compatible with `vfs::Error`, as they are directly wrapped in `get_attr::Fail`.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type wrapped in `get_attr::Fail`. This encompasses standard NFS errors like `StaleFile`, `IO`, `NoEntry`, etc.

Error Propagation Strategy:
- **Direct Propagation**: Errors from internal operations (`path_for_handle`, `metadata`) are not caught or transformed; they are returned immediately as `vfs::Error` inside the `Fail` struct.

Recoverability:
- **Dependent on Error**:
  - `StaleFile`: Indicates the handle is invalid; the client must perform a new lookup.
  - `IO`: Indicates a transient or permanent disk issue; the client may retry or abort.
  - `NoEntry`: Indicates the file does not exist at the resolved path.

Panics:
- **Allowed**: No. The implementation uses `match` and `return` to handle all Result branches explicitly.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::get_attr::GetAttr`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **enable the `MirrorFS` backend to serve file attributes to the NFS server**. The system contains a complex architecture where the `nfs_mamont` server defines a generic Virtual File System (VFS) interface to abstract storage operations. `MirrorFS` is a specific implementation of this VFS that mirrors a local directory structure. This module is necessary because it provides the concrete logic for the `GETATTR` operation, translating the abstract, handle-based requests of the NFS protocol into concrete path-based filesystem queries on the host operating system.

A typical usage scenario of the system involves an NFS client requesting the attributes (size, permissions, modification time) of a file. The `nfs_mamont` RPC layer receives the request, extracts the file handle, and calls the `get_attr` method on the `MirrorFS` instance. This module's implementation then executes: it looks up the local path corresponding to the handle, queries the OS for the file's metadata (stat), converts that metadata into the NFS `Attr` format, and returns it to the client.

Inside the system, the following things happen and they use this module:
1.  **Protocol Translation**: The module acts as the adapter between the "wire format" (handles) and the "system format" (paths). It relies on `path_for_handle` to perform this translation.
2.  **Metadata Normalization**: The module relies on `attr_from_metadata` to ensure that the specific details of the host OS's file metadata are correctly mapped to the standardized fields defined by the NFSv3 specification (e.g., converting `SystemTime` to NFS `nfstime3`).
3.  **Error Standardization**: By propagating errors directly as `vfs::Error`, the module ensures that filesystem-specific failures (like "File not found") are correctly reported to the client using standard NFS status codes.

**Uncertainty**: The public interfaces JSON for `mirrorfs::fs` does not list `path_for_handle`, `metadata`, or `attr_from_metadata`. However, the source code explicitly uses `self.path_for_handle(...).await`, `Self::metadata(...)`, and `Self::attr_from_metadata(...)`. Therefore, the specification assumes these methods exist on `MirrorFS` or its impl block, likely as private or internal methods, to support this functionality.