<!-- SPEC_HASH: ae15310a89e0fa72360ed14a5cb2f6b48138d34a11a45cd4d0adf95c807926e1 -->
# Module Specification

Module: mirrorfs::fs::path_conf_impl
Rust File: mirror_fs/src/fs/path_conf_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs`**: Used to access the `MAX_NAME_LEN` constant, which defines the maximum length for filename components supported by the VFS layer. This constant is used to populate the `name_max` field in the response.
- **`nfs_mamont::vfs::path_conf`**: Used to import the `PathConf` trait and its associated types (`Args`, `Success`, `Fail`). This module provides the concrete implementation of this trait for the `MirrorFS` struct, enabling the backend to respond to NFSv3 `PATHCONF` requests.
- **`super::MirrorFS`**: The struct for which the `PathConf` trait is implemented. The implementation relies on internal methods of `MirrorFS` (specifically `path_for_handle` and `file_attr`) to perform the necessary lookups and metadata retrieval.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `PathConf` for `MirrorFS`

**Intent:**
To provide the logic required to handle the NFSv3 `PATHCONF` procedure for the `MirrorFS` backend. This involves resolving an opaque file handle to a concrete path, retrieving the file's attributes, and returning a set of static configuration limits that describe the behavior of the file system.

**Inputs:**
- `args: path_conf::Args`: Contains the `file::Handle` that the client is querying.

**Outputs:**
- `Result<path_conf::Success, path_conf::Fail>`:
  - `Success`: Contains the file attributes and the path configuration limits.
  - `Fail`: Contains an error code and optionally the file attributes (though currently set to `None` on error).

**Steps:**
1. **Handle Resolution**: The method calls `self.path_for_handle(&args.file).await`. This is an internal method of `MirrorFS` that maps the NFS file handle to a local filesystem path.
2. **Error Handling**: If `path_for_handle` returns an `Err`, the method immediately returns `path_conf::Fail` containing the error and `file_attr: None`.
3. **Attribute Retrieval**: If the handle is resolved successfully, the method calls `Self::file_attr(&path)` to retrieve the current metadata (attributes) for the file.
4. **Response Construction**: The method constructs a `path_conf::Success` struct with the following hardcoded values:
   - `link_max`: `u32::MAX` (Indicates no practical limit on hard links).
   - `name_max`: `vfs::MAX_NAME_LEN` cast to `u32`.
   - `no_trunc`: `true` (The server will reject names longer than `name_max` rather than truncating them).
   - `chown_restricted`: `true` (Only the privileged user can change ownership).
   - `case_insensitive`: `false` (The filesystem is case-sensitive).
   - `case_preserving`: `true` (The case of names is preserved).
5. **Return**: The `Success` struct is returned to the caller.

**Edge Cases:**
- **Invalid Handle**: If the provided file handle cannot be resolved by `path_for_handle`, the operation fails immediately without attempting to fetch attributes.

**Complexity:**
- **Time**: Dependent on the complexity of `path_for_handle` (likely O(1) or O(depth) for handle lookup) and `file_attr` (likely a syscall or metadata cache lookup).
- **Space**: O(1) for the returned structures.

**Determinism:**
- **Deterministic**: Given a valid handle, the returned configuration limits are static constants. The attributes depend on the file system state.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::path_conf`**:
  - **`PathConf` Trait**: Defines the asynchronous `path_conf` method signature. This module implements this trait, allowing `MirrorFS` to be used wherever a `PathConf` implementation is required (e.g., as part of the `Vfs` super-trait).
  - **`Args`, `Success`, `Fail`**: These structures define the input and output contract. The implementation uses `Args` to receive the handle and constructs `Success` or `Fail` to send the response.

- **From `nfs_mamont::vfs`**:
  - **`MAX_NAME_LEN`**: A constant representing the maximum filename length supported by the server. This module uses it to enforce and report the `name_max` limit to the client.

- **From `mirrorfs::fs` (Assumed)**:
  - **`path_for_handle`**: An internal method (not listed in public facts but used in code) responsible for converting a `file::Handle` into a local `Path`.
  - **`file_attr`**: An internal method (not listed in public facts but used in code) responsible for retrieving `file::Attr` metadata for a given local `Path`.

---

## 4. Data Model

Entities:
- This module defines no new entities. It implements logic for existing entities defined in `nfs_mamont::vfs::path_conf`.

Relations:
- **Implementation**: `MirrorFS` implements `path_conf::PathConf`.

Global Invariants:
- **Static Configuration**: The implementation returns static values for configuration flags (`no_trunc`, `chown_restricted`, `case_insensitive`, `case_preserving`). This implies that `MirrorFS` does not support varying these parameters per directory or mount point; they are global properties of this implementation.

## 5. Error Model

Error Types:
- **`path_conf::Fail`**: Wraps a `vfs::Error`.

Error Propagation Strategy:
- **Direct Mapping**: Errors returned by the internal `path_for_handle` method are wrapped directly into `path_conf::Fail`. The `file_attr` field in the `Fail` struct is explicitly set to `None` in the error path.

Recoverability:
- **Dependent on `vfs::Error`**: If the error is `StaleFile` or `BadFileHandle`, the client must perform a new lookup. If it is an `IO` error, the client may retry.

Panics:
- **Allowed**: No. The implementation uses `match` and `return` to handle errors gracefully.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::path_conf::PathConf`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **implement the NFSv3 `PATHCONF` procedure for the `MirrorFS` storage backend**. In the context of the `nfs_mamont` server, the `MirrorFS` struct acts as a concrete implementation of the Virtual File System (VFS) interface. The `PathConf` procedure is a standard NFS requirement that allows clients to query the limits and behavioral characteristics of the file system (such as maximum filename length or case sensitivity).

The system contains a complex architecture where storage backends must implement a suite of traits to be compatible with the NFS server. This module is necessary because it provides the specific logic that translates a generic NFS request (a file handle) into the specific data structures and static configuration values that `MirrorFS` supports. Without this implementation, `MirrorFS` would not satisfy the `Vfs` super-trait (which requires `PathConf`), and the server would be unable to start or handle `PATHCONF` requests from clients.

A typical usage scenario of the system involves a client connecting to the server and issuing a `PATHCONF` request on a specific file or directory to determine how to format filenames or handle permissions. The server receives the request, extracts the file handle, and invokes the `path_conf` method implemented in this module. The code resolves the handle to a local path, retrieves the file's current attributes, and returns a response indicating that the file system is case-sensitive, preserves case, does not truncate long names, and restricts ownership changes.

Inside the system, the following things happen and they use this module:
1.  **Trait Satisfaction**: The `MirrorFS` struct aggregates implementations of various NFS procedures. This module ensures that the `PathConf` requirement is met, allowing `MirrorFS` to be passed to the server context as a valid `Vfs` backend.
2.  **Configuration Reporting**: The module hardcodes specific behaviors (e.g., `case_insensitive: false`). This effectively defines the "personality" of the `MirrorFS` export, informing clients that they must respect case sensitivity and filename length limits strictly.
3.  **Handle Resolution**: The module bridges the gap between the abstract `file::Handle` used by the NFS protocol and the concrete file system path used by the backend logic (via the assumed `path_for_handle` method).

**Assumption**: The code relies on `self.path_for_handle` and `Self::file_attr`. These methods are not listed in the public interface facts for `MirrorFS` but are used here. It is assumed they exist as internal or private methods within the `MirrorFS` implementation to handle handle-to-path mapping and metadata retrieval.