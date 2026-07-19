<!-- SPEC_HASH: 72db9374a2fdd3ce0cd5cc2683b02ed744a51c9f7cc07e53aa2962b5fd2cacd4 -->
# Module Specification

Module: mirrorfs::fs::read_link_impl
Rust File: mirror_fs/src/fs/read_link_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs`**: Used to import the `vfs::Error` enum. This is used to construct error responses, specifically `InvalidArgument` for type mismatches and `IO` for filesystem failures.
- **`nfs_mamont::vfs::file`**: Used to import `file::Type` (to check if the target is a `Symlink`), `file::Path` (to wrap the link target string), and `file::Handle` (the input identifier).
- **`nfs_mamont::vfs::read_link`**: Used to import the `ReadLink` trait and its associated types (`Args`, `Success`, `Fail`). This module provides the concrete implementation of this trait for the `MirrorFS` struct.
- **`super::MirrorFS`**: The struct for which the `ReadLink` trait is implemented. The implementation relies on internal methods of `MirrorFS` (assumed to exist based on code usage) such as `path_for_handle`, `metadata`, `attr_from_metadata`, and `io_error_to_vfs`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Symbolic Link Resolution for `MirrorFS`

**Intent:**
To implement the `READLINK` procedure for the `MirrorFS` backend. This involves translating an abstract file handle into a concrete local filesystem path, verifying that the object is indeed a symbolic link, reading the target path from the filesystem, and validating the target before returning it to the VFS layer.

**Inputs:**
- `args: read_link::Args`: Contains the `file::Handle` of the object to be read.

**Outputs:**
- `Result<read_link::Success, read_link::Fail>`:
  - `Success`: Contains the validated target path (`file::Path`) and the symlink's attributes (`file::Attr`).
  - `Fail`: Contains a `vfs::Error` and optionally the symlink's attributes.

**Steps:**
1. **Handle Resolution**: Calls `self.path_for_handle(&args.file).await` to convert the abstract handle into a local filesystem path. If this fails, returns `Fail` with the error and no attributes.
2. **Metadata Retrieval**: Calls `Self::metadata(&path)` to obtain the file's metadata. If this fails, returns `Fail` with the error and no attributes.
3. **Attribute Conversion**: Converts the raw metadata into a `file::Attr` struct using `Self::attr_from_metadata`.
4. **Type Verification**: Checks if `attr.file_type` is `file::Type::Symlink`. If not, returns `Fail` with `vfs::Error::InvalidArgument` and the retrieved attributes.
5. **Link Reading**: Calls `std::fs::read_link(&path)` to read the target of the symlink. If an OS error occurs, maps it to a `vfs::Error` using `Self::io_error_to_vfs` and returns `Fail` with the attributes.
6. **Encoding Validation**: Converts the target path (an `OsString`) to a UTF-8 string. If it is not valid UTF-8, returns `Fail` with `vfs::Error::IO` and the attributes.
7. **Path Validation**: Attempts to create a `file::Path` from the string using `file::Path::new`. If validation fails (e.g., length limits), returns `Fail` with `vfs::Error::InvalidArgument` and the attributes.
8. **Success**: Returns `Success` containing the validated `file::Path` and the attributes.

**Edge Cases:**
- **Non-Symlink Handle**: If the handle points to a regular file or directory, the operation fails with `InvalidArgument` but returns the attributes of the non-link object.
- **Non-UTF-8 Target**: If the symlink target contains invalid UTF-8 sequences, the operation fails with `vfs::Error::IO`.
- **Blocking I/O**: The implementation uses `std::fs::read_link`, which is a blocking call. In an asynchronous context, this will block the executor thread.

**Complexity:**
- **Time**: O(1) or O(D) where D is the depth of the directory structure (depending on `path_for_handle` implementation), plus the time taken by the OS to read the symlink.
- **Space**: O(L) where L is the length of the symlink target path.

**Determinism:**
- **Deterministic**: The output is strictly determined by the state of the filesystem at the time of the call.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::read_link`**:
 - **`ReadLink` Trait**: This module implements the `read_link` method defined by this trait. It adheres to the contract specifying that `InvalidArgument` must be returned if the target is not a symlink.
 - **`Success` and `Fail` Types**: The module constructs these specific types to return the result. It ensures that `symlink_attr` is populated with `Some(attr)` whenever metadata is successfully retrieved, even if the subsequent read operation fails.

- **From `nfs_mamont::vfs::file`**:
 - **`file::Path` Validation**: The module uses `file::Path::new` to validate the string content of the symlink target. This ensures that the path returned to the client conforms to the system's length and format constraints.
 - **`file::Type`**: Used to strictly check the file type against `Type::Symlink` before attempting to read the link content.

- **From `nfs_mamont::vfs`**:
 - **`vfs::Error`**: Used to standardize error reporting. The module maps OS-level I/O errors (via `io_error_to_vfs`) and logical errors (like type mismatches) to this enum.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on `Args`, `Success`, and `Fail` defined in `nfs_mamont::vfs::read_link`.

Relations:
- **Implementation**: `MirrorFS` implements `read_link::ReadLink`.

Global Invariants:
- **Attribute Consistency**: If `metadata` retrieval succeeds, `symlink_attr` in the returned `Success` or `Fail` struct must always be `Some`.
- **Type Safety**: The function guarantees that if `Success` is returned, the data corresponds to a valid `file::Path` object.

## 5. Error Model

Error Types:
- **`read_link::Fail`**: The wrapper struct containing the error details.
 - **`vfs::Error`**: The specific error variant.
 - **`InvalidArgument`**: Returned if the handle does not refer to a symlink, or if the symlink target string is invalid for `file::Path`.
 - **`IO`**: Returned for filesystem read errors or if the symlink target is not valid UTF-8.

Error Propagation Strategy:
- **Early Return**: The function uses a "fail-fast" approach. If any step (handle resolution, metadata read, type check, link read, validation) fails, it immediately returns a `Fail` struct.
- **Attribute Preservation**: Unlike some error paths where attributes might be unavailable, this implementation attempts to provide attributes (`symlink_attr: Some(attr)`) in the `Fail` struct whenever the metadata was successfully read, even if the link read itself failed.

Recoverability:
- **Dependent on Variant**:
 - `IO`: Potentially transient (e.g., disk busy), though the use of blocking I/O makes recovery harder in an async runtime.
 - `InvalidArgument`: Permanent (client error).

Panics:
- **Allowed**: No.
- **Conditions**: The code uses `match` and `if let` extensively to handle `Result` and `Option` types, avoiding explicit panics on error paths.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::read_link::ReadLink`**: Implemented for `super::MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete filesystem logic for reading symbolic links** within the `MirrorFS` backend. The system contains a layered architecture where the top-level `nfs_mamont` server defines generic NFS procedures (traits), and specific backends like `MirrorFS` provide the implementations that interact with the actual storage. This module is necessary because it bridges the gap between the abstract VFS concepts (file handles, generic errors) and the concrete OS filesystem operations (paths, `std::fs::read_link`).

A typical usage scenario of the system involves an NFS client sending a `READLINK` request for a specific file handle. The request reaches the `MirrorFS` backend. This module takes over, resolving the opaque handle to a local path on the server's disk. It then performs a series of checks: ensuring the file exists, ensuring it is actually a symbolic link, and ensuring the link target is readable and valid UTF-8. If all checks pass, it packages the target path into a `file::Path` struct and returns it to the client.

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: The module enforces the NFSv3 requirement that `READLINK` is only valid for symlinks. If the handle points to a regular file, it explicitly returns `InvalidArgument`, preventing the server from returning garbage data or crashing.
2.  **Data Sanitization**: The module uses `file::Path::new` to validate the link target. This ensures that the server does not relay paths that exceed the NFS protocol limits or contain invalid characters to the client.
3.  **Error Context**: By populating `symlink_attr` in failure cases (where possible), the module allows the client to update its cache with the symlink's metadata even if the read operation failed, which is crucial for maintaining cache consistency in distributed file systems.

**Uncertainty**: The `*.facts.json` for `mirrorfs::fs` does not list the methods `path_for_handle`, `metadata`, `attr_from_metadata`, or `io_error_to_vfs`. However, the code in this module invokes them as `self.path_for_handle(...).await`, `Self::metadata(...)`, etc. Therefore, the specification assumes these methods exist as internal or private helpers within the `MirrorFS` struct or its super-module. The behavior of `path_for_handle` is assumed to be an asynchronous lookup converting a `Handle` to a system path, and `metadata` is assumed to be a synchronous filesystem stat operation.