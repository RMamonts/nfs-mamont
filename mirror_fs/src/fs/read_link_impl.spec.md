<!-- SPEC_HASH: 72db9374a2fdd3ce0cd5cc2683b02ed744a51c9f7cc07e53aa2962b5fd2cacd4 -->
# Module Specification

Module: mirrorfs::fs::read_link_impl
Rust File: mirror_fs/src/fs/read_link_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs`**: Used to import the `vfs::Error` enum, which is required to construct error responses (e.g., `InvalidArgument`, `IO`) when the operation fails or encounters invalid state.
- **`nfs_mamont::vfs::file`**: Used to import `file::Type` (to check if the target is a `Symlink`), `file::Path` (to wrap and validate the link target string), and `file::Handle` (the input type).
- **`nfs_mamont::vfs::read_link`**: Used to import the `ReadLink` trait, along with its `Args`, `Success`, and `Fail` types, to define the interface implementation.
- **`super::MirrorFS`**: The parent struct for which this implementation is provided. The implementation relies on several internal methods of `MirrorFS` (assumed to be defined in `super`) such as `path_for_handle`, `metadata`, `attr_from_metadata`, and `io_error_to_vfs` to bridge the gap between NFS handles and the local filesystem.
- **`std::fs`**: Used via `std::fs::read_link` to perform the actual synchronous read of the symbolic link from the local operating system filesystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `ReadLink` for `MirrorFS`

**Intent:**
To provide the concrete logic for the NFSv3 `READLINK` procedure within the `MirrorFS` backend. This involves translating an abstract file handle into a local filesystem path, verifying the object is a symbolic link, reading its target, and ensuring the result conforms to VFS constraints.

**Inputs:**
- `&self`: A reference to the `MirrorFS` instance.
- `args: read_link::Args`: Arguments containing the `file::Handle` of the symbolic link to be read.

**Outputs:**
- `Result<read_link::Success, read_link::Fail>`:
    - `Success`: Contains the validated target path (`file::Path`) and the symlink's attributes (`file::Attr`).
    - `Fail`: Contains a `vfs::Error` and optionally the symlink's attributes if they could be retrieved.

**Steps:**
1. **Handle Resolution**: Calls `self.path_for_handle(&args.file).await` to convert the NFS file handle into a local filesystem path. If this fails, returns `Fail` with the error and no attributes.
2. **Metadata Retrieval**: Calls `Self::metadata(&path)` to obtain the file's metadata from the local filesystem. If this fails, returns `Fail` with the error and no attributes.
3. **Attribute Conversion**: Converts the raw metadata into a VFS `Attr` structure using `Self::attr_from_metadata(&meta)`.
4. **Type Validation**: Checks if `attr.file_type` matches `file::Type::Symlink`. If not, returns `Fail` with `vfs::Error::InvalidArgument` and the retrieved attributes.
5. **Link Reading**: Calls `std::fs::read_link(&path)` to read the target of the symbolic link. If an I/O error occurs, maps it to a `vfs::Error` using `Self::io_error_to_vfs` and returns `Fail` with the attributes.
6. **String Conversion**: Converts the target path (a `std::path::PathBuf`) to a string. If it is not valid UTF-8, returns `Fail` with `vfs::Error::IO` and the attributes.
7. **Path Validation**: Attempts to create a `file::Path` from the target string using `file::Path::new`. This validates the string against VFS limits (e.g., length). If validation fails, returns `Fail` with `vfs::Error::InvalidArgument` and the attributes.
8. **Success**: Returns `Success` containing the validated `file::Path` and the `file::Attr`.

**Edge Cases:**
- **Non-Symlink Handle**: If the handle points to a regular file or directory, the operation fails with `InvalidArgument` but returns the attributes to allow cache updates.
- **Invalid UTF-8 Target**: If the symlink target on disk contains non-UTF-8 characters, the operation fails with `vfs::Error::IO`.
- **Path Too Long**: If the symlink target string exceeds `MAX_PATH_LEN`, `file::Path::new` fails, resulting in `vfs::Error::InvalidArgument`.

**Complexity:**
- **Time**: O(1) for logic, dominated by filesystem I/O (handle lookup, metadata read, symlink read).
- **Space**: O(L) where L is the length of the symlink target string.

**Determinism:**
- **Deterministic**: The output is strictly determined by the state of the filesystem corresponding to the input handle.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::read_link`**:
    - **`ReadLink` Trait**: Defines the asynchronous interface `read_link` that this module implements. It enforces the contract that the implementation must return `Success` or `Fail` structures.
    - **`Args`, `Success`, `Fail`**: Provides the data structures used for input and output. The `Fail` structure specifically allows returning attributes even on failure, which this module utilizes when metadata retrieval succeeds but the operation fails (e.g., wrong type).

- **From `nfs_mamont::vfs::file`**:
    - **`file::Path::new`**: Used to validate the symlink target string. This ensures that the path returned to the client adheres to the system's maximum path length constraints, preventing protocol violations.
    - **`file::Type::Symlink`**: Used to verify the file type before attempting to read the link content. This ensures the implementation adheres to the NFSv3 requirement that `READLINK` is only valid on symlinks.

- **From `nfs_mamont::vfs`**:
    - **`vfs::Error`**: Used to categorize failures. Specifically, `InvalidArgument` is used for type mismatches or invalid path strings, while `IO` is used for filesystem access failures or encoding issues.

---

## 4. Data Model

Entities:
- This module defines no public entities. It operates entirely on types defined in its dependencies (`read_link::Args`, `read_link::Success`, `read_link::Fail`, `file::Path`, `file::Attr`).

Relations:
- None defined in this module.

Global Invariants:
- None defined in this module.

---

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type returned within `read_link::Fail`.
    - `InvalidArgument`: Returned if the target file is not a symlink or if the symlink target path is invalid (e.g., too long).
    - `IO`: Returned for filesystem I/O errors or if the symlink target cannot be converted to UTF-8.
    - Other errors (e.g., `NoEntry`, `StaleFile`) may be propagated from `path_for_handle` or `metadata` calls.

Error Propagation Strategy:
- **Early Return**: The function uses a series of `match` statements to check intermediate results. If any step fails (handle resolution, metadata retrieval, type check, read operation, validation), it immediately returns a `read_link::Fail` struct wrapping the error.

Recoverability:
- **Dependent on Error**:
    - `InvalidArgument`: Indicates a client error (requesting a read on a non-symlink). Retrying with the same arguments will fail.
    - `IO`: May indicate a transient issue (e.g., disk busy) or a permanent one (e.g., corruption). Retrying might be successful if the issue is transient.

Panics:
- **Allowed**: No.
- **Conditions**: The code handles all potential error paths (handle resolution, metadata, IO, UTF-8 conversion, validation) by returning `Result` types. No `unwrap` or `expect` calls are present in the provided snippet.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::read_link::ReadLink`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **implement the `READLINK` NFSv3 procedure for the `MirrorFS` backend**. The system contains a complex architecture where storage backends must conform to the abstract `Vfs` interface defined in `nfs_mamont`. This module is necessary because it provides the concrete logic that translates the abstract request (read a symlink by handle) into specific actions on the local filesystem (resolve handle, check type, read link, validate result).

A typical usage scenario of the system involves a client requesting the target of a symbolic link. The RPC layer receives the request, extracts the file handle, and invokes the `read_link` method on the `MirrorFS` instance. This module's implementation then executes: it looks up the local path corresponding to the handle, verifies that the file is indeed a symbolic link (returning an error if it is a regular file or directory), reads the target path from the disk, and validates that the target path is a valid UTF-8 string and conforms to NFS path length limits. Finally, it returns the validated path and the file's attributes to the client.

Inside the system, the following things happen and they use this module:
1.  **Handle Translation**: The module relies on `self.path_for_handle` (assumed to be part of `MirrorFS`) to map the opaque NFS handle to a concrete filesystem path. This decouples the network protocol representation from the server's internal storage layout.
2.  **Protocol Enforcement**: The module strictly enforces the NFSv3 specification by checking `file_type` against `file::Type::Symlink`. If the handle refers to any other type of object, it returns `vfs::Error::InvalidArgument`, ensuring the server does not return garbage data for non-symlink objects.
3.  **Data Sanitization**: By passing the symlink target string through `file::Path::new`, the module ensures that any data returned to the client has already been validated against system constraints (like maximum path length), preventing potential protocol errors or buffer overflows in the client or RPC layer.

**Uncertainty**: The implementation relies on internal methods of `MirrorFS` (`path_for_handle`, `metadata`, `attr_from_metadata`, `io_error_to_vfs`) which are not defined in the provided code or dependency facts. Their behavior is inferred from their usage in this module (e.g., `path_for_handle` is async and resolves a handle to a path; `metadata` is synchronous and returns filesystem metadata). Additionally, the use of `std::fs::read_link` (a blocking call) inside an `async` function suggests that this implementation might block the executor thread unless `MirrorFS` is configured to run blocking operations in a separate thread pool (e.g., via `spawn_blocking`), though this mechanism is not visible in the current snippet.