<!-- SPEC_HASH: ae15310a89e0fa72360ed14a5cb2f6b48138d34a11a45cd4d0adf95c807926e1 -->
# Module Specification

Module: mirrorfs::fs::path_conf_impl
Rust File: mirror_fs/src/fs/path_conf_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::path_conf`**: Used to import the `PathConf` trait and its associated types (`Args`, `Success`, `Fail`). This module provides the concrete implementation of this trait for the `MirrorFS` struct, enabling the VFS layer to query file system configuration limits.
- **`nfs_mamont::vfs`**: Used to access the `MAX_NAME_LEN` constant. This constant is used to populate the `name_max` field in the response, ensuring the server reports a consistent maximum filename length defined by the VFS layer.
- **`super::MirrorFS`**: Used as the implementing type. The logic relies on internal methods of `MirrorFS` (specifically `path_for_handle` and `file_attr`) to resolve abstract file handles to concrete paths and retrieve file system attributes.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `PathConf` for `MirrorFS`

**Intent:**
To provide the specific logic required by the `MirrorFS` backend to respond to NFS `PATHCONF` requests. This involves resolving an opaque file handle to a local path, retrieving the file's attributes, and returning static configuration limits that define the behavior of the `MirrorFS` file system.

**Inputs:**
- `args: path_conf::Args`: Contains the `file::Handle` of the object being queried.

**Outputs:**
- `Result<path_conf::Success, path_conf::Fail>`: Returns a structure containing path configuration limits and attributes on success, or an error on failure.

**Steps:**
1. **Handle Resolution**: The method calls `self.path_for_handle(&args.file).await`. This is an internal operation of `MirrorFS` that maps the NFS file handle to a local filesystem path.
2. **Error Handling**: If `path_for_handle` returns an `Err` (e.g., `StaleFile`, `IO`), the method immediately returns `path_conf::Fail` containing the error and sets `file_attr` to `None`.
3. **Attribute Retrieval**: If the handle is resolved successfully, the method calls `Self::file_attr(&path)` to obtain the current metadata for the file.
4. **Response Construction**: The method constructs a `path_conf::Success` struct with:
   - `file_attr`: The attributes retrieved in the previous step.
   - `link_max`: Set to `u32::MAX` (indicating effectively unlimited hard links).
   - `name_max`: Set to `vfs::MAX_NAME_LEN`.
   - `no_trunc`: Set to `true` (the server will reject names exceeding `name_max` rather than truncating them).
   - `chown_restricted`: Set to `true` (only privileged users can change ownership).
   - `case_insensitive`: Set to `false`.
   - `case_preserving`: Set to `true`.
5. **Return**: The `Success` struct is wrapped in `Ok` and returned.

**Edge Cases:**
- **Invalid Handle**: If the provided `file` handle cannot be resolved to a path, the operation fails immediately without attempting to fetch attributes.
- **Attribute Fetch Failure**: If `Self::file_attr` fails, the code currently does not explicitly handle this potential error (it assumes `Self::file_attr` returns an `Option<Attr>` or similar, though the signature suggests it returns `Attr` directly based on usage). *Uncertainty: The signature of `Self::file_attr` is not visible in the provided context, but its usage implies it returns a value compatible with `file_attr` field in `Success` (which is `Option<file::Attr>`).*

**Complexity:**
- **Time**: Dominated by the `path_for_handle` and `file_attr` operations, which likely involve filesystem lookups (syscalls like `stat` or `fstat`). O(1) relative to the filesystem structure, but dependent on OS latency.
- **Space**: O(1) for the returned structures.

**Determinism:**
- **Deterministic**: Given a valid handle and a stable filesystem state, the returned configuration limits are static constants, and the attributes are deterministic properties of the file.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::path_conf`**:
 - **`PathConf` Trait**: Defines the `path_conf` method signature that this module implements. It enforces the contract that the implementation must accept `Args` and return a `Result<Success, Fail>`.
 - **`Args` and `Success` Structs**: Define the input and output data shapes. The `Success` struct specifically requires fields like `link_max`, `name_max`, and boolean flags, which this module populates with specific values for `MirrorFS`.

- **From `nfs_mamont::vfs`**:
 - **`MAX_NAME_LEN`**: A constant used to define the `name_max` field. This ensures that the `MirrorFS` implementation adheres to the global filename length constraints defined by the VFS abstraction layer.

- **From `mirrorfs::fs` (Assumed)**:
 - **`MirrorFS::path_for_handle`**: Although not in the public JSON, this mechanism is critical. It is responsible for the internal mapping of the abstract `Handle` to a concrete `Path`. The correctness of the `path_conf` implementation depends entirely on this method's ability to validate the handle.
 - **`MirrorFS::file_attr`**: Assumed internal mechanism used to populate the `file_attr` field in the response, allowing the client to update its cache.

---

## 4. Data Model

Entities:
- This module defines no new entities. It implements logic for existing entities defined in `nfs_mamont::vfs::path_conf`.

Relations:
- **Implementation**: `MirrorFS` implements `path_conf::PathConf`.

Global Invariants:
- **Static Configuration**: The values for `no_trunc`, `chown_restricted`, `case_insensitive`, and `case_preserving` are hardcoded constants in this implementation. This implies that `MirrorFS` does not support dynamic configuration of these flags; they are fixed properties of this specific file system implementation.
- **Link Limit**: `link_max` is always reported as `u32::MAX`, suggesting the backend does not enforce a specific limit on hard links other than the type's maximum value.

## 5. Error Model

Error Types:
- **`path_conf::Fail`**: The error wrapper returned to the caller. It contains a `vfs::Error` and an optional `file_attr`.

Error Propagation Strategy:
- **Direct Propagation**: Errors returned by `self.path_for_handle` (which are assumed to be `vfs::Error`) are wrapped directly into `path_conf::Fail`.
- **Attribute Omission on Error**: When an error occurs, `file_attr` is explicitly set to `None` in the returned `Fail` struct.

Recoverability:
- **Dependent on `vfs::Error`**: If the error is `StaleFile`, the client must perform a new lookup. If it is `IO`, the client may retry.

Panics:
- **Allowed**: No explicit panics are introduced in this code.
- **Conditions**: If `Self::file_attr` were to panic (e.g., due to an internal logic error in `MirrorFS`), that panic would propagate. However, the implementation itself does not invoke `panic!` or `unwrap()`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::path_conf::PathConf`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete implementation of the NFS `PATHCONF` procedure for the `MirrorFS` backend**. In the broader system, `nfs_mamont` defines the abstract interfaces (traits) that an NFS server must support, while `mirrorfs` provides a specific implementation that mirrors a local filesystem. This module bridges the gap: it takes the abstract request defined by the VFS layer and translates it into the specific behavior and constraints of the `MirrorFS` implementation.

The system contains a complex architecture where storage backends are abstracted behind traits. The `MirrorFS` struct needs to inform clients about the limits and characteristics of the file system it exposes (e.g., how long filenames can be, whether it cares about case). This module is necessary because it defines *what* those limits are for `MirrorFS`. Without this module, the `MirrorFS` struct would not satisfy the `Vfs` super-trait (which requires `PathConf`), and the server could not function as a complete NFSv3 endpoint.

A typical usage scenario of the system involves a client connecting to the server and querying the properties of a directory before performing operations. The client sends a `PATHCONF` request with a file handle. The RPC layer dispatches this to the `path_conf` method implemented here. The code resolves the handle to a local path using `MirrorFS`'s internal logic. It then returns a response indicating that the filesystem is case-sensitive, case-preserving, restricts `chown`, and rejects filenames longer than `vfs::MAX_NAME_LEN`.

Inside the system, the following things happen and they use this module:
1.  **Trait Satisfaction**: The `MirrorFS` struct aggregates implementations of various VFS traits. This module provides the `PathConf` piece, allowing `MirrorFS` to be passed to the generic server logic in `nfs_mamont::lib` as a valid `Vfs` implementation.
2.  **Constraint Enforcement**: By hardcoding `no_trunc: true`, this module dictates the server's behavior when a client attempts to create a file with a name that is too long. The server will reject the operation with an error rather than silently truncating the name, ensuring data integrity and preventing ambiguous filenames.
3.  **Metadata Caching**: By populating the `file_attr` field in the `Success` response, the module allows the client to update its attribute cache for the file handle without needing to issue a separate `GETATTR` call, optimizing network traffic.

**Uncertainty**: The code relies on `self.path_for_handle` and `Self::file_attr`. These methods are not listed in the public interface JSON for `MirrorFS`. It is assumed they are internal helper methods (`pub(crate)` or private) that handle the translation between the VFS's abstract handles and the local filesystem's paths and metadata. The exact behavior of these helpers (e.g., how they handle symlinks or permission errors during path resolution) influences the error results of this module but is defined outside this file.